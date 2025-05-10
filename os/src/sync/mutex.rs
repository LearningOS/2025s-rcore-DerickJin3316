//! Mutex (spin-like and blocking(sleep))

use super::UPSafeCell;
use crate::task::TaskControlBlock;
use crate::task::block_current_and_run_next;
use crate::task::{current_task, wakeup_task};
use alloc::vec::Vec;
use alloc::{collections::VecDeque, sync::Arc};

/// Blocking Mutex struct
pub struct MutexBlocking {
    /// inner
    pub inner: UPSafeCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
    /// avail
    pub avail: isize,
    /// alloc
    pub allocation: Vec<isize>,
    /// need
    pub need: Vec<isize>,
}

impl MutexBlocking {
    /// Create a new blocking mutex
    pub fn new() -> Self {
        trace!("kernel: MutexBlocking::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                    avail: 1 as isize,
                    allocation: Vec::new(),
                    need: Vec::new(),
                })
            },
        }
    }

    /// lock the blocking mutex
    pub fn lock(&self) {
        trace!("kernel: MutexBlocking::lock");
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            block_current_and_run_next();
        } else {
            mutex_inner.locked = true;
        }
    }

    /// unlock the blocking mutex
    pub fn unlock(&self) {
        trace!("kernel: MutexBlocking::unlock");
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            wakeup_task(waking_task);
        } else {
            mutex_inner.locked = false;
        }
    }

    /// lock the blocking mutex
    pub fn lock_tid(&self, tid: usize) {
        trace!("kernel: MutexBlocking::lock");
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            mutex_inner.need[tid] += 1;
            drop(mutex_inner);
            block_current_and_run_next();
        } else {
            mutex_inner.locked = true;
            mutex_inner.avail -= 1;
            mutex_inner.allocation[tid] += 1;
        }
    }

    /// unlock the blocking mutex
    pub fn unlock_tid(&self, tid: usize) {
        trace!("kernel: MutexBlocking::unlock");
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            let wakeup_tid = waking_task
                .inner_exclusive_access()
                .res
                .as_ref()
                .unwrap()
                .tid;
            wakeup_task(waking_task);
            mutex_inner.need[wakeup_tid] -= 1;
        } else {
            mutex_inner.locked = false;
            mutex_inner.avail += 1;
            mutex_inner.allocation[tid] -= 1;
        }
    }

    /// set tid
    pub fn set_tid(&self, tid: usize) {
        let mut inner = self.inner.exclusive_access();
        while inner.allocation.len() < tid + 1 {
            inner.allocation.push(0);
        }
        inner.allocation[tid] = 0;
        while inner.need.len() < tid + 1 {
            inner.need.push(0);
        }
        inner.need[tid] = 0;
    }
}
