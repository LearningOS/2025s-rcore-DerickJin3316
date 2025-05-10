use crate::sync::{Condvar, MutexBlocking, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
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
    let mutex: Option<Arc<MutexBlocking>> = if !blocking {
        Some(Arc::new(MutexBlocking::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    };
    if let Some(mutex) = process_inner.mutex_list[id as usize].as_ref() {
        process_inner
            .tasks
            .iter()
            .flatten()
            .for_each(|task| {
                let tid = task.inner_exclusive_access()
                    .res
                    .as_ref()
                    .unwrap()
                    .tid;
                mutex.set_tid(tid);
            });
    }
    id
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
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let mutex_avail = mutex.inner.exclusive_access().avail;
    if process_inner.deadlock_detect && mutex_avail == 0 {
        let mut work: Vec<isize> = Vec::new();
        let mut all_allocation: Vec<Vec<isize>> = Vec::new();
        let mut all_need: Vec<Vec<isize>> = Vec::new();
        process_inner
            .mutex_list
            .iter()
            .enumerate()
            .filter(|(_, one_mutex)| one_mutex.is_some())
            .for_each(|(_, one_mutex)| {
                let mutex_inner = one_mutex.as_ref().unwrap().inner.exclusive_access();
                work.push(mutex_inner.avail);
            });
            process_inner
                .tasks
                .iter()
                .enumerate()
                .filter(|(_, task)| task.is_some())
                .map(|(one_tid, _)| one_tid)
                .for_each(|one_tid| {
                    all_allocation.push(Vec::new());
                    all_need.push(Vec::new());
                    process_inner
                        .mutex_list
                        .iter()
                        .enumerate()
                        .filter(|(_, one_mutex)| one_mutex.is_some())
                        .for_each(|(one_mutex_id, one_mutex)| {
                            let mutex_inner = one_mutex.as_ref().unwrap().inner.exclusive_access();
                            all_allocation
                                .last_mut()
                                .unwrap()
                                .push(mutex_inner.allocation[one_tid]);
                            all_need
                                .last_mut()
                                .unwrap()
                                .push(if one_mutex_id == mutex_id && one_tid == tid {mutex_inner.need[one_tid] + 1} else {mutex_inner.need[one_tid]});
                        });
                });
        let mut finish = vec![false; all_allocation.len()];
        for _ in 0..finish.len() {
            if let Some(pos) = (0..finish.len())
                .find(|i| {
                    let can_proceed = all_need[*i]
                        .iter()
                        .zip(work.iter())
                        .all(|(&need_j, &work_j)| need_j <= work_j);
                    can_proceed && !finish[*i]
                }) {
                work
                    .iter_mut()
                    .zip(all_allocation[pos].iter())
                    .for_each(|(work_j, &alloc_j)| *work_j += alloc_j);
                finish[pos] = true;
            }
            else {
                return -0xDEAD;
            }
        }
    }
    drop(process_inner);
    drop(process);
    mutex.lock_tid(tid);
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
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    mutex.unlock_tid(tid);
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
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };
    if let Some(sem) = process_inner.semaphore_list[id].as_ref() {
        process_inner
            .tasks
            .iter()
            .flatten()
            .for_each(|task| {
                let tid = task.inner_exclusive_access()
                    .res
                    .as_ref()
                    .unwrap()
                    .tid;
                sem.set_tid(tid);
            });
    }
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
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    sem.up(tid);
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
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    let sem_avail = sem.inner.exclusive_access().avail;
    if process_inner.deadlock_detect && sem_avail == 0 {
        let mut work: Vec<isize> = Vec::new();
        let mut all_allocation: Vec<Vec<isize>> = Vec::new();
        let mut all_need: Vec<Vec<isize>> = Vec::new();
        process_inner
            .semaphore_list
            .iter()
            .enumerate()
            .filter(|(_, one_sem)| one_sem.is_some())
            .for_each(|(_, one_sem)| {
                let sem_inner = one_sem.as_ref().unwrap().inner.exclusive_access();
                work.push(sem_inner.avail);
            });
            process_inner
                .tasks
                .iter()
                .enumerate()
                .filter(|(_, task)| task.is_some())
                .map(|(one_tid, _)| one_tid)
                .for_each(|one_tid| {
                    all_allocation.push(Vec::new());
                    all_need.push(Vec::new());
                    process_inner
                        .semaphore_list
                        .iter()
                        .enumerate()
                        .filter(|(_, one_sem)| one_sem.is_some())
                        .for_each(|(one_sem_id, one_sem)| {
                            let sem_inner = one_sem.as_ref().unwrap().inner.exclusive_access();
                            all_allocation
                                .last_mut()
                                .unwrap()
                                .push(sem_inner.allocation[one_tid]);
                            all_need
                                .last_mut()
                                .unwrap()
                                .push(if one_sem_id == sem_id && one_tid == tid {sem_inner.need[one_tid] + 1} else {sem_inner.need[one_tid]});
                        });
                });
        let mut finish = vec![false; all_allocation.len()];
        for _ in 0..finish.len() {
            if let Some(pos) = (0..finish.len())
                .find(|i| {
                    let can_proceed = all_need[*i]
                        .iter()
                        .zip(work.iter())
                        .all(|(&need_j, &work_j)| need_j <= work_j);
                    can_proceed && !finish[*i]
                }) {
                work
                    .iter_mut()
                    .zip(all_allocation[pos].iter())
                    .for_each(|(work_j, &alloc_j)| *work_j += alloc_j);
                finish[pos] = true;
            }
            else {
                return -0xDEAD;
            }
        }
    }
    drop(process_inner);
    sem.down(tid);
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
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect NOT IMPLEMENTED");
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.deadlock_detect = true;
    0
}
