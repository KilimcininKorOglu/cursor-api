use alloc::sync::Arc;
use atomic_enum::{Atomic, Ordering, atomic_enum};
use core::{future::Future, time::Duration};
use tokio::{sync::Notify, time::sleep};

/// 任务状态机
///
/// 状态流转路径:
/// 1. 正常执行: Scheduled -> Running -> Finished
/// 2. 任务取消: Scheduled -> Cancelled
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
                    // 原子地尝试从 Scheduled 切换到 Running。
                    // Use AcqRel 建立内存屏障：Ensure看到 cancel() 的修改，或让 cancel() 看到我们的修改。
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
                // 等待取消信号，If state 已被 cancel() 修改，这里会被唤醒
                _ = task_inner.signal.notified() => {}
            }
        });

        Self { inner }
    }

    /// 尝试取消任务。
    ///
    /// If任务已经在运行或已完成，取消将Failed。
    pub fn cancel(&self) -> bool {
        // 解决竞态条件：只Have当前状态确认To Scheduled 时才切换To Cancelled。
        // 此处 AcqRel 保证了与任务线程中状态切换的互斥性。
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
