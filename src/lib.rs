//! TODO

#![no_std]

/// Derive macro for [PersistedKey]
#[cfg(feature = "derive")]
pub use persisted_derive::PersistedKey;

use core::{
    any,
    fmt::{self, Debug},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// TODO
/// TODO note about infallibilty
pub trait PersistedStore<K: PersistedKey> {
    /// Load a persisted value from the store, identified by the given key.
    /// Return `Ok(None)` if the value isn't present.
    fn load_persisted(key: &K) -> Option<K::Value>;

    /// Persist a value in the store, under the given key
    fn store_persisted(key: &K, value: K::Value);
}

/// A wrapper for any value that will automatically persist it to the state DB.
/// The value will be loaded from the DB on creation, and saved to the DB on
/// drop.
///
/// ## Generic Params
///
/// - `S`: The backend type used to persist data. While we don't need access to
///   the backend itself here, we do need to know its type so we can access it
///   using [PersistedStore::with_instance] on setup/drop.
/// - `K`: The type of the persistence key. The associated `Value` type will be
///   the type of the contained value.
pub struct Persisted<S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey,
{
    backend: PhantomData<S>,
    key: K,
    /// This is an option so we can move the value out and pass it to the store
    /// during drop
    /// Invariant: Always `Some` until drop
    value: Option<K::Value>,
}

impl<S, K> Persisted<S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey,
{
    /// Initialize a new persisted value. The latest persisted value will be
    /// loaded from the store. If missing, use the given default instead.
    pub fn new(key: K, default: K::Value) -> Self {
        // Fetch persisted value from the backend
        let value = S::load_persisted(&key).unwrap_or(default);

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

// Needed to omit Debug bound on B
impl<S, K> Debug for Persisted<S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey + Debug,
    K::Value: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Persisted")
            .field("backend", &self.backend)
            .field("key", &self.key)
            .field("value", &self.value)
            .finish()
    }
}

// Needed to omit Default bound on B
impl<S, K> Default for Persisted<S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey + Default,
    K::Value: Default,
{
    fn default() -> Self {
        Self::new(Default::default(), Default::default())
    }
}

impl<S, K> Deref for Persisted<S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey,
{
    type Target = K::Value;

    fn deref(&self) -> &Self::Target {
        // Safe because value is always Some until drop
        self.value.as_ref().unwrap()
    }
}

impl<S, K> DerefMut for Persisted<S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safe because value is always Some until drop
        self.value.as_mut().unwrap()
    }
}

/// Save value on drop
impl<S, K> Drop for Persisted<S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey,
{
    fn drop(&mut self) {
        let value = self.value.take().unwrap();
        S::store_persisted(&self.key, value);
    }
}

/// TODO
/// TODO de-dupe code with Persisted
pub struct PersistedLazy<S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value>,
{
    backend: PhantomData<S>,
    key: K,
    container: C,
}

impl<S, K, C> PersistedLazy<S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value>,
{
    /// Load the latest persisted value from the DB. If present, set the value
    /// of the container.
    pub fn new(key: K, mut container: C) -> Self {
        // Fetch persisted value from the backend
        if let Some(value) = S::load_persisted(&key) {
            container.set_persisted(value);
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

// Needed to omit Debug bound on B
impl<S, K, C> Debug for PersistedLazy<S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey + Debug,
    C: PersistedContainer<Value = K::Value> + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PersistedLazy")
            .field("backend", &self.backend)
            .field("key", &self.key)
            .field("container", &self.container)
            .finish()
    }
}

// Needed to omit Default bound on B
impl<S, K, C> Default for PersistedLazy<S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey + Default,
    C: PersistedContainer<Value = K::Value> + Default,
{
    fn default() -> Self {
        Self::new(Default::default(), Default::default())
    }
}

impl<S, K, C> Deref for PersistedLazy<S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.container
    }
}

impl<S, K, C> DerefMut for PersistedLazy<S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.container
    }
}

/// Save value on drop
impl<S, K, C> Drop for PersistedLazy<S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value>,
{
    fn drop(&mut self) {
        let value = self.container.get_persisted();
        S::store_persisted(&self.key, value);
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
    /// program. This method allows you to include the key type, so you could
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

    /// Set the container's value, based on value loaded from the store
    fn set_persisted(&mut self, value: Self::Value);
}

/// TODO
/// TODO add caveat about using types like Option<T>
#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
