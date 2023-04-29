//! Process management syscalls
use core::mem::size_of;

use crate::{
    config::{MAX_SYSCALL_NUM, PAGE_SIZE},
    mm::{translated_byte_buffer, PhysAddr, VirtAddr, VirtPageNum},
    task::{
        change_program_brk, current_user_token, exit_current_and_run_next, get_current_start_time,
        get_current_syscall_times, mmap, munmap, suspend_current_and_run_next, translate,
        TaskStatus,
    },
    // timer::{get_time, get_time_us, CLOCK_FREQ, MSEC_PER_SEC},
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

const TASK_INFO_SIZE: usize = size_of::<TaskInfo>();

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let ts_virt_addr: VirtAddr = (ts as usize).into();
    let ts_page_offset = ts_virt_addr.page_offset();
    let ts_vpn: VirtPageNum = ts_virt_addr.floor();
    // println!("{:?}, {:?}, {:?}", ts_virt_addr, ts_page_offset, ts_vpn);

    // let ts_usec_virt_addr: VirtAddr = (ts as usize + 8).into();
    // let ts_usec_vpn: VirtPageNum = ts_usec_virt_addr.floor();
    // if ts_usec_vpn != ts_vpn {
    //     println!("page diff: {:?}", ts_virt_addr);
    // }

    let ts_pte = match translate(ts_vpn) {
        Some(ts_pte) => ts_pte,
        None => return -1,
    };
    let ts_phys_addr: PhysAddr = ts_pte.ppn().into();
    let ts_addr = ts_phys_addr.0 | ts_page_offset;
    let _ts = ts_addr as *mut TimeVal;
    // let us = get_time_us();
    // unsafe {
    //     *ts = TimeVal {
    //         sec: us / 1_000_000,
    //         usec: us % 1_000_000,
    //     };
    // }

    // println!("{:?}, {:?}", (us / 1_000_000), (us % 1_000_000));
    // println!("{:?}, {:?}", (us / 1_000_000).to_le_bytes(), (us % 1_000_000).to_le_bytes());
    // let ts = translated_byte_buffer(current_user_token(), ts as usize as *const u8, 1);
    let buffers = translated_byte_buffer(
        current_user_token(),
        ts_virt_addr.0 as *const u8,
        size_of::<TimeVal>(),
    );
    // println!("{}", buffers.len());
    // for buffer in buffers {
    //     println!("TimeVal: {}", buffer.len());
    //     println!("{:?}", buffer);
    // }
    // println!("");
    let us = get_time_us();
    let time_val = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    unsafe {
        let time_val_bytes = core::slice::from_raw_parts(
            (&time_val as *const TimeVal) as *const u8,
            size_of::<TimeVal>(),
        );
        // println!("len: {:?}", time_val_bytes.len());
        let mut idx = 0;
        for buffer in buffers {
            for i in 0..buffer.len() {
                buffer[i] = time_val_bytes[idx];
                idx += 1;
            }
        }
    }
    // let bytes = [
    //     (us / 1_000_000).to_le_bytes(),
    //     (us % 1_000_000).to_le_bytes(),
    // ]
    // .concat();
    // let mut idx = 0;
    // for buffer in buffers {
    //     for i in 0..buffer.len() {
    //         buffer[i] = bytes[idx].clone();
    //         idx += 1;
    //     }
    // }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info NOT IMPLEMENTED YET!");
    let ti_virt_addr: VirtAddr = (ti as usize).into();
    let ti_page_offset = ti_virt_addr.page_offset();
    let ti_vpn: VirtPageNum = ti_virt_addr.floor();
    // println!("{:?}, {:?}", ti_virt_addr, ti_vpn);

    // let ti_usec_virt_addr: VirtAddr = (ti as usize + 2016).into();
    // let ti_usec_vpn: VirtPageNum = ti_usec_virt_addr.floor();
    // println!("{:?}, {:?}", ti_usec_virt_addr, ti_usec_vpn);

    let ti_pte = match translate(ti_vpn) {
        Some(ti_pte) => {
            if !ti_pte.is_valid() {
                return -1;
            }
            ti_pte
        }
        None => return -1,
    };
    let ti_phys_addr: PhysAddr = ti_pte.ppn().into();
    let ti_addr = ti_phys_addr.0 | ti_page_offset;
    let _ti = ti_addr as *mut TaskInfo;

    let start_time = match get_current_start_time() {
        Some(start_time) => start_time,
        None => {
            return -1;
        }
    };
    // let current_time = get_time();
    // println!("{} - {} = {}", current_time / (CLOCK_FREQ / MSEC_PER_SEC), start_time / (CLOCK_FREQ / MSEC_PER_SEC), (current_time - start_time) / (CLOCK_FREQ / MSEC_PER_SEC));
    // unsafe {
    //     *ti = TaskInfo {
    //         status: TaskStatus::Running,
    //         syscall_times: get_current_syscall_times(),
    //         time: (current_time - start_time) / (CLOCK_FREQ / MSEC_PER_SEC),
    //     }
    // }

    let buffers = translated_byte_buffer(
        current_user_token(),
        ti_virt_addr.0 as *const u8,
        size_of::<TaskInfo>(),
    );
    // println!("{}", size_of::<TaskInfo>());
    // println!("{}", size_of::<TaskStatus>());
    // println!("{}", size_of::<usize>());
    // println!("{}", size_of::<[u32; 500]>());
    // println!("{}", buffers.len());
    // for buffer in buffers {
    //     println!("TimeVal: {}", buffer.len());
    //     println!("{:?}", buffer);
    // }
    // println!("");
    // let syscall_times_bytes: [u8; MAX_SYSCALL_NUM * 4] = get_current_syscall_times()
    //     .iter()
    //     .flat_map(|x| x.to_le_bytes())
    //     .collect();
    // let mut syscall_times_bytes = [0_u8; MAX_SYSCALL_NUM * 4];
    // let mut idx = 0;
    // for syscall_time in get_current_syscall_times() {
    //     for byte in syscall_time.to_le_bytes() {
    //         syscall_times_bytes[idx] = byte;
    //     }
    //     idx += 1;
    // }
    // let bytes = [
    //     (TaskStatus::Running as usize).to_le_bytes(),
    //     syscall_times_bytes,
    //     ((current_time - start_time) / (CLOCK_FREQ / MSEC_PER_SEC)).to_le_bytes(),
    // ]
    // .concat();

    // let current_time = get_time();
    let current_time = get_time_us();
    // println!("{}, {}, {}, {}", current_time, get_time(), get_time() / (CLOCK_FREQ / 1000), get_time() / (CLOCK_FREQ / 1000000));
    // println!("current_time: {}, {}", current_time / 1000, get_time() / (CLOCK_FREQ / MSEC_PER_SEC));
    // println!("{} - {} = {}", current_time / (CLOCK_FREQ / MSEC_PER_SEC), start_time / (CLOCK_FREQ / MSEC_PER_SEC), (current_time - start_time) / (CLOCK_FREQ / MSEC_PER_SEC));
    let task_info = TaskInfo {
        status: TaskStatus::Running,
        syscall_times: get_current_syscall_times(),
        // time: (current_time - start_time) / (CLOCK_FREQ / MSEC_PER_SEC),
        time: (current_time - start_time) / 1000,
    };
    unsafe {
        let task_info_bytes = core::slice::from_raw_parts(
            (&task_info as *const TaskInfo) as *const u8,
            TASK_INFO_SIZE,
        );
        println!("len: {:?}", task_info_bytes.len());
        let mut idx = 0;
        for buffer in buffers {
            for i in 0..buffer.len() {
                buffer[i] = task_info_bytes[idx];
                idx += 1;
            }
        }
    }

    // let mut bytes = [0_u8; TASK_INFO_SIZE];
    // let mut idx = 0;
    // // for byte in (TaskStatus::Running as usize).to_le_bytes() {
    // //     bytes[idx] = byte;
    // //     idx += 1;
    // // }
    // let task_status = (TaskStatus::Running as usize).to_le_bytes();
    // while idx < 3 {
    //     bytes[idx] = task_status[idx];
    //     idx += 1;
    // }
    // for syscall_time in get_current_syscall_times() {
    //     for byte in syscall_time.to_le_bytes() {
    //         bytes[idx] = byte;
    //         idx += 1;
    //     }
    // }
    // for byte in ((current_time - start_time) / (CLOCK_FREQ / MSEC_PER_SEC)).to_le_bytes() {
    //     bytes[idx] = byte;
    //     idx += 1;
    // }
    // // println!("idx: {}", idx);

    // let mut idx = 0;
    // for buffer in buffers {
    //     for i in 0..buffer.len() {
    //         buffer[i] = bytes[idx].clone();
    //         idx += 1;
    //     }
    // }
    // let task_status = ti_addr as *mut TaskStatus;
    // unsafe {
    //     *task_status = TaskStatus::Running;
    // }
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    if start / PAGE_SIZE * PAGE_SIZE != start {
        return -1;
    }
    // let len = (len + PAGE_SIZE - 1) / PAGE_SIZE * PAGE_SIZE;
    if mmap(start, len, port).is_some() {
        return 0;
    }
    -1
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    if start / PAGE_SIZE * PAGE_SIZE != start {
        return -1;
    }
    if munmap(start, len) {
        0
    } else {
        -1
    }
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
