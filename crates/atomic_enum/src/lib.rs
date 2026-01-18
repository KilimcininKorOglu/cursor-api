#![feature(const_trait_impl)]
#![no_std]

// re-export
pub use core::sync::atomic::Ordering;
pub use traits::{AtomicRepr, WithArithmeticOps, WithBitwiseOps};
pub use wrapper::Atomic;

/// 定义原子操作的能力接口与契约
mod traits {
    use core::sync::atomic::Ordering;

    /// 基础原子存储能力 (Load/Store/CAS)
    pub trait AtomicStorage: Sized {
        type Primitive: Copy;

        fn load(&self, order: Ordering) -> Self::Primitive;
        fn store(&self, val: Self::Primitive, order: Ordering);
        fn swap(&self, val: Self::Primitive, order: Ordering) -> Self::Primitive;

        fn compare_exchange(
            &self,
            current: Self::Primitive,
            new: Self::Primitive,
            success: Ordering,
            failure: Ordering,
        ) -> Result<Self::Primitive, Self::Primitive>;

        fn compare_exchange_weak(
            &self,
            current: Self::Primitive,
            new: Self::Primitive,
            success: Ordering,
            failure: Ordering,
        ) -> Result<Self::Primitive, Self::Primitive>;

        fn fetch_update<F>(
            &self,
            set_order: Ordering,
            fetch_order: Ordering,
            f: F,
        ) -> Result<Self::Primitive, Self::Primitive>
        where
            F: FnMut(Self::Primitive) -> Option<Self::Primitive>;
    }

    /// 位运算能力 (Bitwise)
    pub trait AtomicBitwise: AtomicStorage {
        fn fetch_and(&self, val: Self::Primitive, order: Ordering) -> Self::Primitive;
        fn fetch_nand(&self, val: Self::Primitive, order: Ordering) -> Self::Primitive;
        fn fetch_or(&self, val: Self::Primitive, order: Ordering) -> Self::Primitive;
        fn fetch_xor(&self, val: Self::Primitive, order: Ordering) -> Self::Primitive;
    }

    /// 算术运算能力 (Arithmetic)
    /// 注意：枚举和布尔值不应实现此 Trait，以防语义错误。
    pub trait AtomicArithmetic: AtomicStorage {
        fn fetch_add(&self, val: Self::Primitive, order: Ordering) -> Self::Primitive;
        fn fetch_sub(&self, val: Self::Primitive, order: Ordering) -> Self::Primitive;
        fn fetch_max(&self, val: Self::Primitive, order: Ordering) -> Self::Primitive;
        fn fetch_min(&self, val: Self::Primitive, order: Ordering) -> Self::Primitive;
    }

    /// 桥接高层类型 T 与底层原子存储 Storage
    pub const unsafe trait AtomicRepr: Copy {
        type Storage: AtomicStorage;

        fn const_new(val: Self) -> Self::Storage;
        fn into_prim(self) -> <Self::Storage as AtomicStorage>::Primitive;

        /// SAFETY: 调用者必须保证 `val` 是该类型 T 的有效位模式
        unsafe fn from_prim(val: <Self::Storage as AtomicStorage>::Primitive) -> Self;
    }

    /// 标记 Trait：启用位运算方法
    pub trait WithBitwiseOps: AtomicRepr
    where Self::Storage: AtomicBitwise
    {
    }

    /// 标记 Trait：启用算术运算方法
    pub trait WithArithmeticOps: AtomicRepr
    where Self::Storage: AtomicArithmetic
    {
    }
}

/// 核心封装结构体
mod wrapper {
    use super::traits::*;
    use core::{marker::PhantomData, sync::atomic::Ordering};

    /// A generic atomic type wrapping a value of type `T`.
    ///
    /// This struct provides a type-safe wrapper around the standard library's atomic primitives
    /// (like [`AtomicU8`], [`AtomicUsize`], etc.), allowing atomic operations on user-defined
    /// types such as `enum`s or newtype wrappers, provided they implement [`AtomicRepr`].
    ///
    /// # Layout
    ///
    /// `Atomic<T>` is `#[repr(transparent)]`. It has the exact same in-memory representation
    /// as the underlying atomic storage (`T::Storage`). For example, `Atomic<MyEnum>` (backed by `u8`)
    /// has the same layout as [`AtomicU8`].
    ///
    /// # Threading Model
    ///
    /// Atomic variables are safe to share between threads (they implement [`Sync`]).
    /// All operations take an [`Ordering`] argument which represents the strength of
    /// the memory barrier.
    ///
    /// # Validity and Safety
    ///
    /// While `Atomic<T>` ensures that all operations performed *through* its API preserve
    /// the validity of `T` (e.g., ensuring an `enum` always holds a valid discriminant),
    /// it cannot protect against external unsafe code writing invalid raw values into the
    /// underlying memory.
    ///
    /// If the underlying storage is modified non-atomically (or via raw pointers) to a value
    /// that is not a valid bit-pattern for `T`, subsequent `load` operations may result in
    /// **undefined behavior**.
    ///
    /// # Capabilities
    ///
    /// All types `T` support basic operations:
    /// - [`load`](Atomic::load)
    /// - [`store`](Atomic::store)
    /// - [`swap`](Atomic::swap)
    /// - [`compare_exchange`](Atomic::compare_exchange)
    ///
    /// Additional operations are available depending on the traits implemented by `T`:
    /// - **Bitwise operations** (`fetch_and`, etc.) are available if `T: WithBitwiseOps`.
    /// - **Arithmetic operations** (`fetch_add`, etc.) are available if `T: WithArithmeticOps`.
    #[repr(transparent)]
    pub struct Atomic<T: AtomicRepr> {
        inner: T::Storage,
        _marker: PhantomData<T>,
    }

    // 基础功能：所有实现了 AtomicRepr 的类型均可用
    impl<T: AtomicRepr> Atomic<T> {
        /// Creates a new generic atomic.
        ///
        /// # Examples
        ///
        /// ```
        /// use my_crate::{Atomic, atomic_enum};
        ///
        /// #[repr(u8)]
        /// #[derive(Debug, PartialEq, Clone, Copy)]
        /// enum State {
        ///     Idle = 0,
        ///     Running = 1,
        /// }
        /// atomic_enum!(State = u8);
        ///
        /// static STATE: Atomic<State> = Atomic::new(State::Idle);
        /// ```
        #[inline]
        pub const fn new(val: T) -> Self
        where T: [const] AtomicRepr {
            Self { inner: T::const_new(val), _marker: PhantomData }
        }

        /// Loads a value from the atomic.
        ///
        /// `load` takes an [`Ordering`] argument which describes the memory ordering of this operation.
        /// Possible values are [`SeqCst`], [`Acquire`] and [`Relaxed`].
        ///
        /// # Panics
        ///
        /// Panics if `order` is [`Release`] or [`AcqRel`].
        #[inline(always)]
        pub fn load(&self, order: Ordering) -> T {
            // SAFETY: AtomicRepr contract guarantees the storage value is valid for T.
            unsafe { T::from_prim(self.inner.load(order)) }
        }

        /// Stores a value into the atomic.
        ///
        /// `store` takes an [`Ordering`] argument which describes the memory ordering of this operation.
        /// Possible values are [`SeqCst`], [`Release`] and [`Relaxed`].
        ///
        /// # Panics
        ///
        /// Panics if `order` is [`Acquire`] or [`AcqRel`].
        #[inline(always)]
        pub fn store(&self, val: T, order: Ordering) { self.inner.store(val.into_prim(), order); }

        /// Stores a value into the atomic, returning the previous value.
        ///
        /// `swap` takes an [`Ordering`] argument which describes the memory ordering
        /// of this operation. All ordering modes are possible. Note that using
        /// [`Acquire`] makes the store part of this operation [`Relaxed`], and
        /// using [`Release`] makes the load part [`Relaxed`].
        #[inline(always)]
        pub fn swap(&self, val: T, order: Ordering) -> T {
            unsafe { T::from_prim(self.inner.swap(val.into_prim(), order)) }
        }

        /// Stores a value into the atomic if the current value is the same as the `current` value.
        ///
        /// The return value is a result indicating whether the new value was written and
        /// containing the previous value. On success this value is guaranteed to be equal to `current`.
        ///
        /// `compare_exchange` takes two [`Ordering`] arguments to describe the memory
        /// ordering of this operation. `success` describes the required ordering for the
        /// read-modify-write operation that takes place if the comparison with `current` succeeds.
        /// `failure` describes the required ordering for the load operation that takes place when
        /// the comparison fails.
        #[inline(always)]
        pub fn compare_exchange(
            &self,
            current: T,
            new: T,
            success: Ordering,
            failure: Ordering,
        ) -> Result<T, T> {
            match self.inner.compare_exchange(
                current.into_prim(),
                new.into_prim(),
                success,
                failure,
            ) {
                Ok(v) => Ok(unsafe { T::from_prim(v) }),
                Err(v) => Err(unsafe { T::from_prim(v) }),
            }
        }

        /// Stores a value into the atomic if the current value is the same as the `current` value.
        ///
        /// Unlike [`Atomic::compare_exchange`], this function is allowed to spuriously fail even
        /// when the comparison succeeds, which can result in more efficient code on some platforms.
        /// The return value is a result indicating whether the new value was written and containing
        /// the previous value.
        #[inline(always)]
        pub fn compare_exchange_weak(
            &self,
            current: T,
            new: T,
            success: Ordering,
            failure: Ordering,
        ) -> Result<T, T> {
            match self.inner.compare_exchange_weak(
                current.into_prim(),
                new.into_prim(),
                success,
                failure,
            ) {
                Ok(v) => Ok(unsafe { T::from_prim(v) }),
                Err(v) => Err(unsafe { T::from_prim(v) }),
            }
        }

        /// Fetches the value, and applies a function to it that returns an optional
        /// new value. Returns a `Result` of `Ok(previous_value)` if the function returned `Some(_)`,
        /// else `Err(previous_value)`.
        ///
        /// Note: This may call the function multiple times if the value has been changed from other
        /// threads in the meantime, as long as the function returns `Some(_)`.
        pub fn fetch_update<F>(
            &self,
            set_order: Ordering,
            fetch_order: Ordering,
            mut f: F,
        ) -> Result<T, T>
        where
            F: FnMut(T) -> Option<T>,
        {
            let res = self.inner.fetch_update(set_order, fetch_order, |v| {
                f(unsafe { T::from_prim(v) }).map(T::into_prim)
            });
            match res {
                Ok(v) => Ok(unsafe { T::from_prim(v) }),
                Err(v) => Err(unsafe { T::from_prim(v) }),
            }
        }
    }

    // 扩展功能：仅当类型被标记为支持位运算时启用
    impl<T: WithBitwiseOps> Atomic<T>
    where <T as AtomicRepr>::Storage: AtomicBitwise
    {
        /// Bitwise "and" with the current value.
        ///
        /// Performs a bitwise "and" operation on the current value and the argument `val`, and
        /// sets the new value to the result.
        ///
        /// Returns the previous value.
        #[inline(always)]
        pub fn fetch_and(&self, val: T, order: Ordering) -> T {
            unsafe { T::from_prim(self.inner.fetch_and(val.into_prim(), order)) }
        }

        /// Bitwise "nand" with the current value.
        ///
        /// Performs a bitwise "nand" operation on the current value and the argument `val`, and
        /// sets the new value to the result.
        ///
        /// Returns the previous value.
        #[inline(always)]
        pub fn fetch_nand(&self, val: T, order: Ordering) -> T {
            unsafe { T::from_prim(self.inner.fetch_nand(val.into_prim(), order)) }
        }

        /// Bitwise "or" with the current value.
        ///
        /// Performs a bitwise "or" operation on the current value and the argument `val`, and
        /// sets the new value to the result.
        ///
        /// Returns the previous value.
        #[inline(always)]
        pub fn fetch_or(&self, val: T, order: Ordering) -> T {
            unsafe { T::from_prim(self.inner.fetch_or(val.into_prim(), order)) }
        }

        /// Bitwise "xor" with the current value.
        ///
        /// Performs a bitwise "xor" operation on the current value and the argument `val`, and
        /// sets the new value to the result.
        ///
        /// Returns the previous value.
        #[inline(always)]
        pub fn fetch_xor(&self, val: T, order: Ordering) -> T {
            unsafe { T::from_prim(self.inner.fetch_xor(val.into_prim(), order)) }
        }
    }

    // 扩展功能：仅当类型被标记为支持算术运算时启用
    impl<T: WithArithmeticOps> Atomic<T>
    where <T as AtomicRepr>::Storage: AtomicArithmetic
    {
        /// Adds to the current value, returning the previous value.
        ///
        /// This operation wraps around on overflow.
        #[inline(always)]
        pub fn fetch_add(&self, val: T, order: Ordering) -> T {
            unsafe { T::from_prim(self.inner.fetch_add(val.into_prim(), order)) }
        }

        /// Subtracts from the current value, returning the previous value.
        ///
        /// This operation wraps around on overflow.
        #[inline(always)]
        pub fn fetch_sub(&self, val: T, order: Ordering) -> T {
            unsafe { T::from_prim(self.inner.fetch_sub(val.into_prim(), order)) }
        }

        /// Maximum with the current value.
        ///
        /// Finds the maximum of the current value and the argument `val`, and
        /// sets the new value to the result.
        ///
        /// Returns the previous value.
        #[inline(always)]
        pub fn fetch_max(&self, val: T, order: Ordering) -> T {
            unsafe { T::from_prim(self.inner.fetch_max(val.into_prim(), order)) }
        }

        /// Minimum with the current value.
        ///
        /// Finds the minimum of the current value and the argument `val`, and
        /// sets the new value to the result.
        ///
        /// Returns the previous value.
        #[inline(always)]
        pub fn fetch_min(&self, val: T, order: Ordering) -> T {
            unsafe { T::from_prim(self.inner.fetch_min(val.into_prim(), order)) }
        }
    }
}

/// 标准库类型的实现细节
mod impls {
    use super::traits::*;
    use core::sync::atomic::{self, Ordering};

    // 辅助宏：实现 AtomicStorage 和 AtomicBitwise
    macro_rules! impl_common_atomics {
        ($Atom:ty, $Prim:ty) => {
            impl AtomicStorage for $Atom {
                type Primitive = $Prim;
                #[inline(always)]
                fn load(&self, order: Ordering) -> $Prim { self.load(order) }
                #[inline(always)]
                fn store(&self, v: $Prim, order: Ordering) { self.store(v, order) }
                #[inline(always)]
                fn swap(&self, v: $Prim, order: Ordering) -> $Prim { self.swap(v, order) }
                #[inline(always)]
                fn compare_exchange(
                    &self,
                    c: $Prim,
                    n: $Prim,
                    s: Ordering,
                    f: Ordering,
                ) -> Result<$Prim, $Prim> {
                    self.compare_exchange(c, n, s, f)
                }
                #[inline(always)]
                fn compare_exchange_weak(
                    &self,
                    c: $Prim,
                    n: $Prim,
                    s: Ordering,
                    f: Ordering,
                ) -> Result<$Prim, $Prim> {
                    self.compare_exchange_weak(c, n, s, f)
                }
                #[inline(always)]
                fn fetch_update<F>(
                    &self,
                    s: Ordering,
                    f: Ordering,
                    func: F,
                ) -> Result<$Prim, $Prim>
                where
                    F: FnMut($Prim) -> Option<$Prim>,
                {
                    self.fetch_update(s, f, func)
                }
            }

            impl AtomicBitwise for $Atom {
                #[inline(always)]
                fn fetch_and(&self, v: $Prim, order: Ordering) -> $Prim { self.fetch_and(v, order) }
                #[inline(always)]
                fn fetch_nand(&self, v: $Prim, order: Ordering) -> $Prim {
                    self.fetch_nand(v, order)
                }
                #[inline(always)]
                fn fetch_or(&self, v: $Prim, order: Ordering) -> $Prim { self.fetch_or(v, order) }
                #[inline(always)]
                fn fetch_xor(&self, v: $Prim, order: Ordering) -> $Prim { self.fetch_xor(v, order) }
            }
        };
    }

    // Bool 实现：仅 Storage + Bitwise
    impl_common_atomics!(atomic::AtomicBool, bool);

    unsafe impl const AtomicRepr for bool {
        type Storage = atomic::AtomicBool;
        #[inline(always)]
        fn const_new(val: Self) -> Self::Storage { atomic::AtomicBool::new(val) }
        #[inline(always)]
        fn into_prim(self) -> bool { self }
        #[inline(always)]
        unsafe fn from_prim(val: bool) -> Self { val }
    }
    impl WithBitwiseOps for bool {}

    // 整数实现：Storage + Bitwise + Arithmetic
    macro_rules! impl_int_atomics {
        ($($Prim:ty => $Atom:ty),* $(,)?) => {
            $(
                impl_common_atomics!($Atom, $Prim);

                impl AtomicArithmetic for $Atom {
                    #[inline(always)] fn fetch_add(&self, v: $Prim, order: Ordering) -> $Prim { self.fetch_add(v, order) }
                    #[inline(always)] fn fetch_sub(&self, v: $Prim, order: Ordering) -> $Prim { self.fetch_sub(v, order) }
                    #[inline(always)] fn fetch_max(&self, v: $Prim, order: Ordering) -> $Prim { self.fetch_max(v, order) }
                    #[inline(always)] fn fetch_min(&self, v: $Prim, order: Ordering) -> $Prim { self.fetch_min(v, order) }
                }

                unsafe impl const AtomicRepr for $Prim {
                    type Storage = $Atom;
                    #[inline(always)] fn const_new(val: Self) -> Self::Storage { <$Atom>::new(val) }
                    #[inline(always)] fn into_prim(self) -> $Prim { self }
                    #[inline(always)] unsafe fn from_prim(val: $Prim) -> Self { val }
                }

                impl WithBitwiseOps for $Prim {}
                impl WithArithmeticOps for $Prim {}
            )*
        };
    }

    impl_int_atomics! {
        u8   => atomic::AtomicU8,
        i8   => atomic::AtomicI8,
        u16  => atomic::AtomicU16,
        i16  => atomic::AtomicI16,
        u32  => atomic::AtomicU32,
        i32  => atomic::AtomicI32,
        u64  => atomic::AtomicU64,
        i64  => atomic::AtomicI64,
        usize => atomic::AtomicUsize,
        isize => atomic::AtomicIsize,
    }
}

/// 自动实现宏
///
/// 生成的枚举仅实现 `AtomicRepr`，不实现标记 Trait，
/// 从而在编译期禁止对 Enum 进行非法运算。
#[macro_export]
macro_rules! atomic_enum {
    ($Enum:ty = $Base:ty) => {
        const _: () = {
            if ::core::mem::size_of::<$Enum>() != ::core::mem::size_of::<$Base>() {
                panic!(concat!(
                    "[AtomicEnum Error] Size mismatch!\n",
                    "Type: ", stringify!($Enum), "\n",
                    "Base: ", stringify!($Base), "\n",
                    "Hint: Did you forget to add `#[repr(", stringify!($Base), ")]` to your enum?"
                ));
            }

            if ::core::mem::align_of::<$Enum>() != ::core::mem::align_of::<$Base>() {
                panic!(concat!(
                    "[AtomicEnum Error] Alignment mismatch!\n",
                    "Type: ", stringify!($Enum), " vs Base: ", stringify!($Base), "\n",
                    "Ensure the alignment matches the backing integer."
                ));
            }

            unsafe impl const $crate::AtomicRepr for $Enum {
                type Storage = <$Base as $crate::AtomicRepr>::Storage;

                #[inline(always)]
                fn const_new(val: Self) -> Self::Storage {
                    let v = val as $Base;
                    <$Base as $crate::AtomicRepr>::const_new(v)
                }

                #[inline(always)]
                fn into_prim(self) -> $Base { self as $Base }

                #[inline(always)]
                #[allow(unsafe_op_in_unsafe_fn)]
                unsafe fn from_prim(val: $Base) -> Self {
                    // SAFETY: 宏使用者需保证枚举与底层类型的内存布局一致性
                    ::core::mem::transmute(val)
                }
            }
        };
    };
}
