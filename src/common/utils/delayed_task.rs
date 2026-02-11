use alloc::sync::Arc;
use atomic_enum::{Atomic, Ordering, atomic_enum};
use core::{future::Future, time::Duration};
use tokio::{sync::Notify, time::sleep};

/// Task state machine
///
/// State transition paths:
/// 1. Normal execution: Scheduled -> Running -> Finished
/// 2. Task cancellation: Scheduled -> Cancelled
#[repr(u8)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TaskState {
    Scheduled,
    Running,
    Cancelled,
    Finished,
}

atomic_enum!(TaskState = u8);

pub struct DelayedTask {
    inner: Arc<Inner>,
}

struct Inner {
    state: Atomic<TaskState>,
    signal: Notify,
}

impl DelayedTask {
    pub fn new<F, Fut>(delay: Duration, task_fn: F) -> Self
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let inner =
            Arc::new(Inner { state: Atomic::new(TaskState::Scheduled), signal: Notify::new() });

        let task_inner = inner.clone();

        let _handle = tokio::spawn(async move {
            tokio::select! {
                _ = sleep(delay) => {
                    // Atomically try to switch from Scheduled to Running.
                    // Use AcqRel to establish memory barrier: ensure we see cancel()'s modifications, or let cancel() see ours.
                    let res = task_inner.state.compare_exchange(
                        TaskState::Scheduled,
                        TaskState::Running,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );

                    if res.is_ok() {
                        task_fn().await;
                        task_inner.state.store(TaskState::Finished, Ordering::Release);
                    }
                }
                // Wait for cancellation signal, if state has already been modified by cancel(), this will be woken up
                _ = task_inner.signal.notified() => {}
            }
        });

        Self { inner }
    }

    /// Try to cancel the task.
    ///
    /// If the task is already running or already completed, cancellation will fail.
    pub fn cancel(&self) -> bool {
        // Resolve race condition: only switch to Cancelled if current state is confirmed to be Scheduled.
        // AcqRel here ensures mutual exclusion with state switch in task thread.
        let res = self.inner.state.compare_exchange(
            TaskState::Scheduled,
            TaskState::Cancelled,
            Ordering::AcqRel,
            Ordering::Acquire,
        );

        if res.is_ok() {
            self.inner.signal.notify_one();
            true
        } else {
            false
        }
    }

    pub fn state(&self) -> TaskState { self.inner.state.load(Ordering::Acquire) }
}
