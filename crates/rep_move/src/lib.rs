//! Iterator that yields N-1 replications followed by the original value.
//!
//! Optimized for expensive-to-clone types by moving the original on the last iteration.

#![no_std]
#![feature(const_destruct)]
#![feature(const_trait_impl)]

use core::{
    fmt,
    iter::FusedIterator,
    marker::{Destruct, PhantomData},
};

/// Replication strategy for `RepMove`.
///
/// Implementations are distinguished by the argument types `Args`.
pub const trait Replicator<Args, Src, Dst = Src> {
    /// Creates a replica with mutable access to the remaining count.
    fn replicate(&mut self, source: &Src, remaining: &mut usize) -> Dst;
}

// Blanket impl for simple replicators: FnMut(&T) -> T
impl<T, F> const Replicator<(&T,), T> for F
where F: [const] FnMut(&T) -> T
{
    #[inline]
    fn replicate(&mut self, source: &T, remaining: &mut usize) -> T {
        let item = self(source);
        *remaining = remaining.saturating_sub(1);
        item
    }
}

// Blanket impl for state-aware replicators: FnMut(&T, usize) -> T
impl<T, F> const Replicator<(&T, usize), T> for F
where F: [const] FnMut(&T, usize) -> T
{
    #[inline]
    fn replicate(&mut self, source: &T, remaining: &mut usize) -> T {
        let item = self(source, *remaining);
        *remaining = remaining.saturating_sub(1);
        item
    }
}

// Blanket impl for advanced state-aware replicators: FnMut(&T, &mut usize) -> T
impl<T, F> const Replicator<(&T, &mut usize), T> for F
where F: [const] FnMut(&T, &mut usize) -> T
{
    #[inline]
    fn replicate(&mut self, source: &T, remaining: &mut usize) -> T { self(source, remaining) }
}

enum State<T, R> {
    Active { source: T, remaining: usize, rep_fn: R },
    Done,
}

/// Iterator yielding N-1 replicas then the original.
///
/// The behavior is determined by the signature of the `rep_fn` closure.
///
/// # Examples
///
/// Simple cloning `(&T) -> T`:
/// ```
/// # use rep_move::RepMove;
/// let v = vec![1, 2, 3];
/// let mut iter = RepMove::new(v, Vec::clone, 3);
///
/// assert_eq!(iter.next(), Some(vec![1, 2, 3]));
/// assert_eq!(iter.next(), Some(vec![1, 2, 3]));
/// assert_eq!(iter.next(), Some(vec![1, 2, 3])); // moved
/// ```
///
/// Accessing remaining count `(&T, usize) -> T`:
/// ```
/// # use rep_move::RepMove;
/// let s = String::from("item");
/// let mut iter = RepMove::new(
///     s,
///     |s: &String, n| format!("{}-{}", s, n),
///     3
/// );
///
/// assert_eq!(iter.next(), Some("item-2".to_string()));
/// assert_eq!(iter.next(), Some("item-1".to_string()));
/// assert_eq!(iter.next(), Some("item".to_string()));
/// ```
///
/// Modifying remaining count `(&T, &mut usize) -> T`:
/// ```
/// # use rep_move::RepMove;
/// let v = vec![1, 2, 3];
/// let mut iter = RepMove::new(
///     v,
///     |v: &Vec<i32>, remaining: &mut usize| {
///         if v.len() > 10 {
///             *remaining = 0; // Stop early for large vectors
///         } else {
///             *remaining = remaining.saturating_sub(1);
///         }
///         v.clone()
///     },
///     5
/// );
/// // Will yield fewer items due to the custom logic
/// ```
pub struct RepMove<Args, T, R: Replicator<Args, T>> {
    state: State<T, R>,
    _marker: PhantomData<Args>,
}

impl<Args, T, R: Replicator<Args, T>> RepMove<Args, T, R> {
    /// Creates a new replicating iterator.
    #[inline]
    pub const fn new(source: T, rep_fn: R, count: usize) -> Self
    where
        T: [const] Destruct,
        R: [const] Destruct,
    {
        if count == 0 {
            Self { state: State::Done, _marker: PhantomData }
        } else {
            Self {
                state: State::Active { source, remaining: count - 1, rep_fn },
                _marker: PhantomData,
            }
        }
    }
}

impl<Args, T, R: Replicator<Args, T>> Iterator for RepMove<Args, T, R> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let state = core::mem::replace(&mut self.state, State::Done);

        match state {
            State::Active { source, mut remaining, mut rep_fn } => {
                if remaining > 0 {
                    let item = rep_fn.replicate(&source, &mut remaining);
                    self.state = State::Active { source, remaining, rep_fn };
                    Some(item)
                } else {
                    Some(source)
                }
            }
            State::Done => None,
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<Args, T, R: Replicator<Args, T>> ExactSizeIterator for RepMove<Args, T, R> {
    #[inline]
    fn len(&self) -> usize {
        match &self.state {
            State::Active { remaining, .. } => remaining + 1,
            State::Done => 0,
        }
    }
}

impl<Args, T, R: Replicator<Args, T>> FusedIterator for RepMove<Args, T, R> {}

impl<Args, T: fmt::Debug, R: Replicator<Args, T>> fmt::Debug for RepMove<Args, T, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.state {
            State::Active { source, remaining, .. } => f
                .debug_struct("RepMove::Active")
                .field("source", source)
                .field("remaining", remaining)
                .finish_non_exhaustive(),
            State::Done => write!(f, "RepMove::Done"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern crate alloc;

    use alloc::{
        format,
        string::{String, ToString as _},
        vec,
        vec::Vec,
    };

    #[test]
    fn test_simple_clone() {
        let v = vec![1, 2, 3];
        let mut iter = RepMove::new(v, Vec::clone, 3);

        assert_eq!(iter.len(), 3);
        assert_eq!(iter.next(), Some(vec![1, 2, 3]));
        assert_eq!(iter.len(), 2);
        assert_eq!(iter.next(), Some(vec![1, 2, 3]));
        assert_eq!(iter.len(), 1);
        assert_eq!(iter.next(), Some(vec![1, 2, 3]));
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_state_aware() {
        let s = String::from("test");
        let mut iter = RepMove::new(s, |s: &String, n: usize| format!("{s}-{n}"), 2);

        assert_eq!(iter.next(), Some("test-1".to_string()));
        assert_eq!(iter.next(), Some("test".to_string()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_mutable_control() {
        let v = vec![1, 2, 3];
        let mut iter = RepMove::new(
            v,
            |v: &Vec<i32>, remaining: &mut usize| {
                if *remaining > 1 {
                    *remaining = 1; // Skip ahead
                } else {
                    *remaining = remaining.saturating_sub(1);
                }
                v.clone()
            },
            4,
        );

        // Should yield fewer items due to skipping
        assert_eq!(iter.next(), Some(vec![1, 2, 3]));
        assert_eq!(iter.next(), Some(vec![1, 2, 3]));
        assert_eq!(iter.next(), Some(vec![1, 2, 3]));
        assert_eq!(iter.next(), None);
    }
}
