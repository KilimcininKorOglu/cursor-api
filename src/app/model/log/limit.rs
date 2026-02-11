use core::num::NonZeroUsize;

/// Request log limit enumeration
#[derive(Debug, Clone, Copy)]
pub enum LogsLimit {
    /// Disabled日志记录
    Disabled,
    /// HaveLimit的日志记录，参数To最大日志数Amount
    Limited(NonZeroUsize),
}

impl LogsLimit {
    #[inline]
    pub fn from_usize(limit: usize) -> Self {
        match NonZeroUsize::new(limit) {
            None => Self::Disabled,
            Some(n) => Self::Limited(n),
        }
    }

    /// CheckWhetherNeed保存日志
    #[inline(always)]
    pub fn should_log(&self) -> bool { !matches!(self, Self::Disabled) }

    /// Get日志Limit
    #[inline(always)]
    pub fn get_limit(self) -> usize {
        match self {
            Self::Disabled => 0,
            Self::Limited(limit) => limit.get(),
        }
    }
}
