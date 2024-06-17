//! TODO

#![no_std]

use core::{any, fmt::Display, marker::PhantomData};
// TODO remove dep
use derive_more::{Deref, DerefMut};

/// Re-export derive macros
#[cfg(feature = "derive")]
pub use persisted_derive::PersistedKey;

/// TODO
pub trait PersistedStore<K: PersistedKey> {
    /// The type of error that this store can encounter while saving or loading
    /// persisted values
    type Error: Display;

    /// Execute a function with an instance of this store. This is how
    /// `persisted` will access the store. The closure-based interface provides
    /// compatibility with [thread locals](std::thread_local).
    fn with_instance<T>(f: impl FnOnce(&Self) -> T) -> T;

    /// Load a persisted value from the store, identified by the given key.
    /// Return `Ok(None)` if the value isn't present.
    fn load_persisted(&self, key: &K) -> Result<Option<K::Value>, Self::Error>;

    /// Persist a value in the store, under the given key
    fn store_persisted(
        &self,
        key: &K,
        value: K::Value,
    ) -> Result<(), Self::Error>;
}

/// A wrapper for any value that will automatically persist it to the state DB.
/// The value will be loaded from the DB on creation, and saved to the DB on
/// drop.
#[derive(derive_more::Debug)]
pub struct Persisted<B, K>
where
    B: PersistedStore<K>,
    K: PersistedKey,
{
    #[debug(skip)] // This omits the Debug bound on B
    backend: PhantomData<B>,
    key: K,
    /// This is an option so we can move the value out and pass it to the store
    /// during drop
    /// Invariant: Always `Some` until drop
    value: Option<K::Value>,
}

impl<B, K> Persisted<B, K>
where
    B: PersistedStore<K>,
    K: PersistedKey,
{
    /// Initialize a new persisted value. The latest persisted value will be
    /// loaded from the store. If missing, use the given default instead.
    pub fn new(key: K, default: K::Value) -> Self {
        // Fetch persisted value from the backend
        let value = match B::with_instance(|store| store.load_persisted(&key)) {
            Ok(Some(value)) => value,
            Ok(None) => default,
            // TODO tracing
            Err(error) => panic!("{error}"),
        };

        Self {
            backend: PhantomData,
            key,
            value: Some(value),
        }
    }

    /// Initialize a new persisted value. The latest persisted value will be
    /// loaded from the store. If missing, use the value type's [Default]
    /// implementation instead.
    pub fn new_default(key: K) -> Self
    where
        K::Value: Default,
    {
        Self::new(key, K::Value::default())
    }
}

impl<B, K> Deref for Persisted<B, K>
where
    B: PersistedStore<K>,
    K: PersistedKey,
{
    type Target = K::Value;

    fn deref(&self) -> &Self::Target {
        // Safe because value is always Some until drop
        self.value.as_ref().unwrap()
    }
}

impl<B, K> DerefMut for Persisted<B, K>
where
    B: PersistedStore<K>,
    K: PersistedKey,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safe because value is always Some until drop
        self.value.as_mut().unwrap()
    }
}

impl<B, K> Default for Persisted<B, K>
where
    B: PersistedStore<K>,
    K: PersistedKey + Default,
    K::Value: Default,
{
    fn default() -> Self {
        Self::new(Default::default(), Default::default())
    }
}

/// TODO
impl<B, K> PartialEq<K::Value> for Persisted<B, K>
where
    B: PersistedStore<K>,
    K: PersistedKey,
    K::Value: PartialEq,
{
    fn eq(&self, other: &K::Value) -> bool {
        self.deref() == other
    }
}

/// Save value on drop
impl<B, K> Drop for Persisted<B, K>
where
    B: PersistedStore<K>,
    K: PersistedKey,
{
    fn drop(&mut self) {
        let value = self.value.take().unwrap();
        if let Err(_error) =
            B::with_instance(|store| store.store_persisted(&self.key, value))
        {
            // TODO tracing
        }
    }
}

/// TODO
/// TODO de-dupe code with Persisted
#[derive(derive_more::Debug, Deref, DerefMut)]
pub struct PersistedLazy<B, K, C>
where
    B: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value>,
{
    #[debug(skip)] // This omits the Debug bound on B
    backend: PhantomData<B>,
    key: K,
    #[deref]
    #[deref_mut]
    container: C,
}

impl<B, K, C> PersistedLazy<B, K, C>
where
    B: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value>,
{
    /// Load the latest persisted value from the DB. If present, set the value
    /// of the container.
    pub fn new(key: K, mut container: C) -> Self {
        // Fetch persisted value from the backend
        match B::with_instance(|store| store.load_persisted(&key)) {
            Ok(Some(value)) => container.set_persisted(value),
            Ok(None) => {}
            // TODO tracing
            Err(error) => panic!("{error}"),
        }

        Self {
            backend: PhantomData,
            key,
            container,
        }
    }

    /// TODO
    pub fn new_default(key: K) -> Self
    where
        C: Default,
    {
        Self::new(key, C::default())
    }
}

impl<B, K, C> Default for PersistedLazy<B, K, C>
where
    B: PersistedStore<K>,
    K: PersistedKey + Default,
    C: PersistedContainer<Value = K::Value> + Default,
{
    fn default() -> Self {
        Self::new(Default::default(), Default::default())
    }
}

/// TODO
/// TODO is this an anti-pattern? Check PartialEq docs
impl<B, K, C> PartialEq<C> for PersistedLazy<B, K, C>
where
    B: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value> + PartialEq,
{
    fn eq(&self, other: &C) -> bool {
        &self.container == other
    }
}

/// Save value on drop
impl<B, K, C> Drop for PersistedLazy<B, K, C>
where
    B: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value>,
{
    fn drop(&mut self) {
        let value = self.container.get_persisted();
        if let Err(_error) =
            B::with_instance(|store| store.store_persisted(&self.key, value))
        {
            // TODO tracing
        }
    }
}

/// TODO
pub trait PersistedKey {
    /// The type of the persisted value associated with this key
    type Value;

    /// Get a unique name for this key type. This should be globally unique
    /// within the scope of your program. This is important to use while
    /// persisting because most serialization formats don't include the name of
    /// the type, just the content. This unique name allows the store to
    /// disambiguate between different key types of the same structure, which is
    /// particular important for unit keys. For example, if your store persists
    /// data as JSON,  a serialized key may be just an ID, e.g. `3`. This alone
    /// is not a useful key because it's ambiguous in the scope of your entire
    /// program. [type_name] allows you to include the key type, so you could
    /// serialize the same key as `["Person", 3]` or `{"type": "Person", "key":
    /// 3}`. It's up to the [PersistedStore] implementation to decide how to
    /// actually employ this function, it's provided merely as a utility.
    ///
    /// In most cases this you can rely on the derive implementation, which uses
    /// [std::any::type_name]. However, for wrapper key types (e.g.
    /// [SingletonKey]), this should return the name of the wrapped type.
    ///
    /// Using this is *not* necessary if you use a persistence format that
    /// includes the type name, e.g. [RON](https://github.com/ron-rs/ron). If
    /// that's the case your implementations of this can return `""` (or panic),
    /// but in most cases it's easier just to use the derive macro anyway, and
    /// just don't call this function.
    fn type_name() -> &'static str;
}

/// A container that can store and provide a persisted value. This is used in
/// conjunction with [PersistedLazy] to define how to lazily get the value that
/// should be persisted, and how to restore state when a persisted value is
/// loaded during initialization.
pub trait PersistedContainer {
    /// The value to be persisted
    type Value;

    /// Get the current value to persist in the store
    fn get_persisted(&self) -> Self::Value;

    /// Set the container's value, based on value loaded from the persistence
    /// store
    fn set_persisted(&mut self, value: Self::Value);
}

/// TODO
/// TODO add caveat about using types like Option<T>
#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SingletonKey<V> {
    phantom: PhantomData<V>,
}

impl<V> PersistedKey for SingletonKey<V> {
    type Value = V;

    fn type_name() -> &'static str {
        any::type_name::<V>()
    }
}

#[cfg(test)]
mod tests {
    // TODO
}
