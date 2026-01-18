use crate::{
    DashMap, Equivalent,
    lock::{RwLockReadGuardDetached, RwLockWriteGuardDetached},
    mapref::one::Ref,
};
use core::{
    hash::{BuildHasher, Hash},
    mem,
};
use hashbrown::hash_table;

#[cfg_attr(feature = "inline-more", inline)]
pub(crate) fn make_hash<Q, S>(hash_builder: &S, val: &Q) -> u64
where
    Q: Hash + ?Sized,
    S: BuildHasher,
{
    hash_builder.hash_one(val)
}

#[cfg_attr(feature = "inline-more", inline)]
pub(crate) fn make_hasher<Q, V, S>(hash_builder: &S) -> impl Fn(&(Q, V)) -> u64 + '_
where
    Q: Hash,
    S: BuildHasher,
{
    move |val| make_hash::<Q, S>(hash_builder, &val.0)
}

impl<K, V, S> DashMap<K, V, S> {
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn raw_entry_mut(&self) -> RawEntryBuilderMut<'_, K, V, S> {
        RawEntryBuilderMut { map: self }
    }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn raw_entry(&self) -> RawEntryBuilder<'_, K, V, S> { RawEntryBuilder { map: self } }
}

pub struct RefMut<'a, K: ?Sized, V: ?Sized> {
    guard: RwLockWriteGuardDetached<'a>,
    pub key: &'a mut K,
    pub value: &'a mut V,
}

impl<'a, K: ?Sized, V: ?Sized> RefMut<'a, K, V> {
    pub(crate) fn new(
        guard: RwLockWriteGuardDetached<'a>,
        key: &'a mut K,
        value: &'a mut V,
    ) -> Self {
        Self { guard, key, value }
    }

    pub fn downgrade(self) -> Ref<'a, K, V> {
        Ref::new(unsafe { RwLockWriteGuardDetached::downgrade(self.guard) }, self.key, self.value)
    }
}

impl<'a, K: core::fmt::Debug + ?Sized, V: core::fmt::Debug + ?Sized> core::fmt::Debug
    for RefMut<'a, K, V>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RefMut").field("key", &self.key).field("value", &self.value).finish()
    }
}

/// A builder for creating a raw entry in a `DashMap`.
pub struct RawEntryBuilderMut<'a, K, V, S> {
    map: &'a DashMap<K, V, S>,
}

impl<'a, K, V, S> RawEntryBuilderMut<'a, K, V, S> {
    /// Access an entry by key.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn from_key<Q>(self, k: &Q) -> RawEntryMut<'a, K, V, S>
    where
        S: BuildHasher,
        K: Hash,
        Q: Hash + Equivalent<K> + ?Sized,
    {
        let hash = self.map.hash_u64(k);
        self.from_key_hashed_nocheck(hash, k)
    }

    /// Access an entry by a pre-computed hash and a key.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn from_key_hashed_nocheck<Q>(self, hash: u64, k: &Q) -> RawEntryMut<'a, K, V, S>
    where
        S: BuildHasher,
        K: Hash,
        Q: Equivalent<K> + ?Sized,
    {
        self.from_hash(hash, |q| k.equivalent(q))
    }

    /// Access an entry by a pre-computed hash and a matching function.
    pub fn from_hash<F>(self, hash: u64, mut is_match: F) -> RawEntryMut<'a, K, V, S>
    where
        S: BuildHasher,
        K: Hash,
        F: FnMut(&K) -> bool,
    {
        let idx = self.map.determine_shard(hash as usize);
        let shard_lock = self.map.shards[idx].write();

        // SAFETY: The guard is stored in the returned RawEntryMut, ensuring the lock
        // is held as long as the entry exists.
        let (shard_guard, table) = unsafe { RwLockWriteGuardDetached::detach_from(shard_lock) };

        match table.find_entry(
            hash,
            |(k, _)| is_match(k),
        ) {
            Ok(entry) => RawEntryMut::Occupied(RawOccupiedEntryMut {
                shard: shard_guard,
                entry,
            }),
            Err(entry) => RawEntryMut::Vacant(RawVacantEntryMut {
                shard: shard_guard,
                table: entry.into_table(),
                hash_builder: &self.map.hasher,
            }),
        }
    }
}

/// A raw entry in the map.
pub enum RawEntryMut<'a, K, V, S> {
    Occupied(RawOccupiedEntryMut<'a, K, V>),
    Vacant(RawVacantEntryMut<'a, K, V, S>),
}

impl<'a, K, V, S> RawEntryMut<'a, K, V, S> {
    /// Sets the value of the entry, and returns an OccupiedEntry.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(self, key: K, value: V) -> RawOccupiedEntryMut<'a, K, V>
    where
        K: Hash,
        S: BuildHasher,
    {
        match self {
            RawEntryMut::Occupied(mut entry) => {
                entry.insert(value);
                entry
            }
            RawEntryMut::Vacant(entry) => entry.insert_entry(key, value),
        }
    }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn or_insert(self, default_key: K, default_val: V) -> RefMut<'a, K, V>
    where
        K: Hash,
        S: BuildHasher,
    {
        match self {
            RawEntryMut::Occupied(entry) => entry.into_key_value(),
            RawEntryMut::Vacant(entry) => entry.insert(default_key, default_val),
        }
    }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn or_insert_with<F>(self, default: F) -> RefMut<'a, K, V>
    where
        F: FnOnce() -> (K, V),
        K: Hash,
        S: BuildHasher,
    {
        match self {
            RawEntryMut::Occupied(entry) => entry.into_key_value(),
            RawEntryMut::Vacant(entry) => {
                let (k, v) = default();
                entry.insert(k, v)
            }
        }
    }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn and_modify<F>(mut self, f: F) -> Self
    where F: FnOnce(&mut K, &mut V) {
        if let RawEntryMut::Occupied(entry) = &mut self {
            let (k, v) = entry.get_key_value_mut();
            f(k, v);
        }
        self
    }
}

pub struct RawOccupiedEntryMut<'a, K, V> {
    shard: RwLockWriteGuardDetached<'a>,
    entry: hash_table::OccupiedEntry<'a, (K, V)>,
    // hash_builder: &'a S,
}

impl<'a, K, V> RawOccupiedEntryMut<'a, K, V> {
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn key(&self) -> &K { &self.entry.get().0 }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn key_mut(&mut self) -> &mut K { &mut self.entry.get_mut().0 }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn into_key(self) -> &'a mut K { &mut self.entry.into_mut().0 }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get(&self) -> &V { &self.entry.get().1 }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn into_mut(self) -> &'a mut V { &mut self.entry.into_mut().1 }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get_mut(&mut self) -> &mut V { &mut self.entry.get_mut().1 }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get_key_value(&self) -> (&K, &V) {
        let (k, v) = self.entry.get();
        (k, v)
    }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get_key_value_mut(&mut self) -> (&mut K, &mut V) {
        let (k, v) = self.entry.get_mut();
        (k, v)
    }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn into_key_value(self) -> RefMut<'a, K, V> {
        let (k, v) = self.entry.into_mut();
        RefMut::new(self.shard, k, v)
    }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(&mut self, value: V) -> V { mem::replace(self.get_mut(), value) }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert_key(&mut self, key: K) -> K { mem::replace(self.key_mut(), key) }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn remove(self) -> V { self.remove_entry().1 }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn remove_entry(self) -> (K, V) { self.entry.remove().0 }

    // #[cfg_attr(feature = "inline-more", inline)]
    // pub fn replace_entry_with<F>(self, f: F) -> RawEntryMut<'a, K, V, S>
    // where F: FnOnce(&K, V) -> Option<V> {
    //     let proxy: hashbrown::hash_map::RawOccupiedEntryMut<'a, K, V, S> = unsafe {
    //         let (bucket, table): (
    //             core::ptr::NonNull<(K, V)>,
    //             &'a mut hash_table::HashTable<(K, V)>,
    //         ) = core::mem::transmute(self.entry);
    //         core::mem::transmute((bucket, table, self.hash_builder))
    //     };
    //     let result = proxy.replace_entry_with(f);
    //     match result {
    //         hashbrown::hash_map::RawEntryMut::Occupied(entry) => {
    //             let (bucket, table, hash_builder): (
    //                 core::ptr::NonNull<(K, V)>,
    //                 &'a mut hash_table::HashTable<(K, V)>,
    //                 &'a S,
    //             ) = unsafe { core::mem::transmute(entry) };
    //             RawEntryMut::Occupied(RawOccupiedEntryMut {
    //                 shard: self.shard,
    //                 entry: unsafe { core::mem::transmute((bucket, table)) },
    //                 hash_builder,
    //             })
    //         }
    //         hashbrown::hash_map::RawEntryMut::Vacant(entry) => {
    //             let (table, hash_builder) = unsafe { core::mem::transmute(entry) };
    //             RawEntryMut::Vacant(RawVacantEntryMut { shard: self.shard, table, hash_builder })
    //         }
    //     }
    // }
}

pub struct RawVacantEntryMut<'a, K, V, S> {
    shard: RwLockWriteGuardDetached<'a>,
    table: &'a mut hash_table::HashTable<(K, V)>,
    hash_builder: &'a S,
}

impl<'a, K, V, S> RawVacantEntryMut<'a, K, V, S> {
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(self, key: K, value: V) -> RefMut<'a, K, V>
    where
        K: Hash,
        S: BuildHasher,
    {
        let hash = make_hash::<K, S>(self.hash_builder, &key);
        self.insert_hashed_nocheck(hash, key, value)
    }

    #[cfg_attr(feature = "inline-more", inline)]
    #[allow(clippy::shadow_unrelated)]
    pub fn insert_hashed_nocheck(self, hash: u64, key: K, value: V) -> RefMut<'a, K, V>
    where
        K: Hash,
        S: BuildHasher,
    {
        let &mut (ref mut k, ref mut v) = self
            .table
            .insert_unique(hash, (key, value), make_hasher::<_, V, S>(self.hash_builder))
            .into_mut();
        RefMut::new(self.shard, k, v)
    }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert_with_hasher<H>(
        self,
        hash: u64,
        key: K,
        value: V,
        hasher: H,
    ) -> (&'a mut K, &'a mut V)
    where
        H: Fn(&K) -> u64,
    {
        let &mut (ref mut k, ref mut v) =
            self.table.insert_unique(hash, (key, value), |x| hasher(&x.0)).into_mut();
        (k, v)
    }

    /// Helper to match hashbrown behavior for API compatibility.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert_entry(self, key: K, value: V) -> RawOccupiedEntryMut<'a, K, V>
    where
        K: Hash,
        S: BuildHasher,
    {
        let hash = make_hash::<K, S>(self.hash_builder, &key);
        let entry =
            self.table.insert_unique(hash, (key, value), make_hasher::<_, V, S>(self.hash_builder));
        RawOccupiedEntryMut { shard: self.shard, entry }
    }
}

pub struct RawEntryBuilder<'a, K, V, S> {
    map: &'a DashMap<K, V, S>,
}

impl<'a, K, V, S> RawEntryBuilder<'a, K, V, S> {
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn from_key<Q>(self, k: &Q) -> Option<Ref<'a, K, V>>
    where
        S: BuildHasher,
        K: Hash,
        Q: Hash + Equivalent<K> + ?Sized,
    {
        let hash = self.map.hash_u64(k);
        self.from_key_hashed_nocheck(hash, k)
    }

    #[cfg_attr(feature = "inline-more", inline)]
    pub fn from_key_hashed_nocheck<Q>(self, hash: u64, k: &Q) -> Option<Ref<'a, K, V>>
    where Q: Equivalent<K> + ?Sized {
        self.from_hash(hash, |q| k.equivalent(q))
    }

    pub fn from_hash<F>(self, hash: u64, mut is_match: F) -> Option<Ref<'a, K, V>>
    where F: FnMut(&K) -> bool {
        let idx = self.map.determine_shard(hash as usize);
        let shard_lock = self.map.shards[idx].read();

        // SAFETY: Detach guard to return Ref which holds the lock.
        let (shard_guard, table) = unsafe { RwLockReadGuardDetached::detach_from(shard_lock) };

        match table.find(hash, |(k, _)| is_match(k)) {
            Some((k, v)) => Some(Ref::new(shard_guard, k, v)),
            None => None,
        }
    }
}
