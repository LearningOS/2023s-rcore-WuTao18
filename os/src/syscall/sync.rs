use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::sync::Arc;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        process_inner.mutex_available[id] = 1;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_available.push(1);
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());

    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    process_inner
        .mutex_need
        .entry(tid)
        .and_modify(|mutex_need| {
            mutex_need
                .entry(mutex_id)
                .and_modify(|need| *need += 1)
                .or_insert(1);
        })
        .or_insert(BTreeMap::from([(mutex_id, 1)]));
    if process_inner.deadlock_detect_enabled {
        let mut work = process_inner.mutex_available.clone();
        let mut unfinished = BTreeSet::<usize>::new();
        for tid in process_inner.mutex_allocation.keys() {
            unfinished.insert(*tid);
        }
        for tid in process_inner.mutex_need.keys() {
            unfinished.insert(*tid);
        }
        loop {
            let mut t = None;
            // step 2
            for unfinished_tid in unfinished.iter() {
                let mut flag = true;
                if let Some(mutex_need) = process_inner.mutex_need.get(unfinished_tid) {
                    for (sid, need) in mutex_need.iter() {
                        if *need > work[*sid] {
                            flag = false;
                            break;
                        }
                    }
                }
                if flag {
                    t = Some(unfinished_tid.clone());
                    break;
                }
            }
            if let Some(t) = t {
                // step 3
                if let Some(mutex_alloc) = process_inner.mutex_allocation.get(&t) {
                    for (sid, alloc) in mutex_alloc.iter() {
                        work[*sid] += *alloc;
                    }
                }
                unfinished.remove(&t);
            } else {
                // step 4
                if unfinished.is_empty() {
                    break;
                } else {
                    return -0xdead;
                }
            }
        }
    }

    drop(process_inner);
    drop(process);
    mutex.lock();

    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.mutex_available[mutex_id] -= 1;
    process_inner
        .mutex_allocation
        .entry(tid)
        .and_modify(|mutex_alloc| {
            mutex_alloc
                .entry(mutex_id)
                .and_modify(|alloc| *alloc += 1)
                .or_insert(1);
        })
        .or_insert(BTreeMap::from([(mutex_id, 1)]));
    process_inner
        .mutex_need
        .entry(tid)
        .and_modify(|mutex_need| {
            mutex_need.entry(mutex_id).and_modify(|need| *need -= 1);
            if let Some(need) = mutex_need.get(&mutex_id) {
                if *need <= 0 {
                    mutex_need.remove(&mutex_id);
                }
            }
        });
    if let Some(mutex_need) = process_inner.mutex_need.get(&tid) {
        if mutex_need.is_empty() {
            process_inner.mutex_need.remove(&tid);
        }
    }
    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());

    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    process_inner
        .mutex_allocation
        .entry(tid)
        .and_modify(|mutex_alloc| {
            mutex_alloc.entry(mutex_id).and_modify(|alloc| *alloc -= 1);
        });
    process_inner.mutex_available[mutex_id] += 1;

    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        process_inner.sem_available[id] = res_count;
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.sem_available.push(res_count);
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    println!("sys_semaphore_up");
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());

    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    process_inner
        .sem_allocation
        .entry(tid)
        .and_modify(|sem_alloc| {
            sem_alloc.entry(sem_id).and_modify(|alloc| *alloc -= 1);
        });
    process_inner.sem_available[sem_id] += 1;

    drop(process_inner);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    process_inner
        .sem_need
        .entry(tid)
        .and_modify(|sem_need| {
            sem_need
                .entry(sem_id)
                .and_modify(|need| *need += 1)
                .or_insert(1);
        })
        .or_insert(BTreeMap::from([(sem_id, 1)]));
    println!(
        "tid: {}, sem_id: {}, sem_need: {:?}, sem_allocation: {:?}",
        tid, sem_id, process_inner.sem_need, process_inner.sem_allocation
    );
    if process_inner.deadlock_detect_enabled {
        let mut work = process_inner.sem_available.clone();
        println!("before work: {:?}", work);
        let mut unfinished = BTreeSet::<usize>::new();
        for task_id in process_inner.sem_allocation.keys() {
            unfinished.insert(*task_id);
        }
        // for task in &process_inner.tasks {
        //     if let Some(task) = task {
        //         if let Some(res) = task.inner_exclusive_access().res.as_ref() {
        //             unfinished.insert(res.tid);
        //         }
        //     }
        // }
        for task_id in process_inner.sem_need.keys() {
            unfinished.insert(*task_id);
        }
        // println!("unfinished: {:?}", unfinished);
        loop {
            let mut t = None;
            // step 2
            for unfinished_tid in unfinished.iter() {
                let mut flag = true;
                if let Some(sem_need) = process_inner.sem_need.get(unfinished_tid) {
                    for (sid, need) in sem_need.iter() {
                        println!(
                            "unfinished_tid: {}, sid: {}, need: {}, work: {:?}",
                            unfinished_tid, *sid, *need, work
                        );
                        // if *need > work[*sid] || work[*sid] == 0 {
                        if *need > work[*sid] {
                            println!("work[*sid]: {:?}", work[*sid]);
                            flag = false;
                            break;
                        }
                    }
                }
                if flag {
                    t = Some(unfinished_tid.clone());
                    break;
                }
            }
            if let Some(t) = t {
                // step 3
                if let Some(sem_alloc) = process_inner.sem_allocation.get(&t) {
                    for (sid, alloc) in sem_alloc.iter() {
                        work[*sid] += *alloc;
                    }
                }
                unfinished.remove(&t);
            } else {
                // step 4
                if unfinished.is_empty() {
                    break;
                } else {
                    println!("deadlock! unfinished: {:?}", unfinished);
                    return -0xdead;
                }
            }
        }
        println!("after work: {:?}", work);
    }
    drop(process_inner);
    sem.down();

    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    println!("sem_available: {:?}", process_inner.sem_available[sem_id]);
    process_inner.sem_available[sem_id] -= 1;
    process_inner
        .sem_allocation
        .entry(tid)
        .and_modify(|sem_alloc| {
            sem_alloc
                .entry(sem_id)
                .and_modify(|alloc| *alloc += 1)
                .or_insert(1);
        })
        .or_insert(BTreeMap::from([(sem_id, 1)]));
    process_inner.sem_need.entry(tid).and_modify(|sem_need| {
        sem_need.entry(sem_id).and_modify(|need| *need -= 1);
        if let Some(need) = sem_need.get(&sem_id) {
            if *need <= 0 {
                sem_need.remove(&sem_id);
            }
        }
    });
    if let Some(sem_need) = process_inner.sem_need.get(&tid) {
        if sem_need.is_empty() {
            process_inner.sem_need.remove(&tid);
        }
    }
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect NOT IMPLEMENTED");
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    match enabled {
        0 => {
            process_inner.enable_deadlock_detect(false);
            0
        }
        1 => {
            process_inner.enable_deadlock_detect(true);
            0
        }
        _ => -1,
    }
}
