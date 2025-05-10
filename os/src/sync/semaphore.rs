//! Semaphore

use crate::sync::UPSafeCell;
use crate::task::{block_current_and_run_next, current_task, wakeup_task, TaskControlBlock};
use alloc::vec::Vec;
use alloc::{collections::VecDeque, sync::Arc};

/// semaphore structure
pub struct Semaphore {
    /// semaphore inner
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
    pub avail: isize,
    pub allocation: Vec<isize>,
    pub need: Vec<isize>,
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(res_count: usize) -> Self {
        trace!("kernel: Semaphore::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                    avail: res_count as isize,
                    allocation: Vec::new(),
                    need: Vec::new(),
                })
            },
        }
    }

    /// up operation of semaphore
    pub fn up(&self, tid: usize) {
        trace!("kernel: Semaphore::up");
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                let wakeup_tid = task
                    .inner_exclusive_access()
                    .res
                    .as_ref()
                    .unwrap()
                    .tid;
                wakeup_task(task);
                inner.need[wakeup_tid] -= 1;
            }
        }
        else {
            inner.avail += 1;
            inner.allocation[tid] -= 1;
        }
    }

    /// down operation of semaphore
    pub fn down(&self, tid: usize) {
        trace!("kernel: Semaphore::down");
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.need[tid] += 1;
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        }
        else {
            inner.avail -= 1;
            inner.allocation[tid] += 1;
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
