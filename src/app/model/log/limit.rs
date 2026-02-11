use core::num::NonZeroUsize;

/// Request log limit enumeration
#[derive(Debug, Clone, Copy)]
pub enum LogsLimit {
    /// Logging disabled
    Disabled,
    /// Logging with limit, parameter is maximum log count
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

    /// Check whether need to save logs
    #[inline(always)]
    pub fn should_log(&self) -> bool { !matches!(self, Self::Disabled) }

    /// Get log limit
    #[inline(always)]
    pub fn get_limit(self) -> usize {
        match self {
            Self::Disabled => 0,
            Self::Limited(limit) => limit.get(),
        }
    }
}
