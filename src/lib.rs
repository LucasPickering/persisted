//! TODO
//!
//! ```
//! TODO hashmap
//! ```

use derive_more::{Deref, DerefMut};
use std::{any, fmt::Display, marker::PhantomData};

/// TODO
pub trait PersistedStore<K: PersistedKey> {
    type Error: Display;

    /// TODO
    fn with_instance<T>(f: impl FnOnce(&Self) -> T) -> T;

    /// TODO rename?
    fn get(&self, key: &K) -> Result<Option<K::Value>, Self::Error>;

    /// TODO rename?
    fn set(&self, key: &K, value: K::Value) -> Result<(), Self::Error>;
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
    /// TODO explain option
    value: Option<K::Value>,
}

impl<B, K> Persisted<B, K>
where
    B: PersistedStore<K>,
    K: PersistedKey,
{
    /// Load the latest persisted value from the DB. If present, set the value
    /// of the container. If not, fall back to the given default
    pub fn new(key: K, default: K::Value) -> Self {
        // Fetch persisted value from the backend
        let value = match B::with_instance(|store| store.get(&key)) {
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

    /// TODO
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
        // TODO explain safety
        self.value.as_ref().unwrap()
    }
}

impl<B, K> DerefMut for Persisted<B, K>
where
    B: PersistedStore<K>,
    K: PersistedKey,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        // TODO explain safety
        self.value.as_mut().unwrap()
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

impl<B, K> Drop for Persisted<B, K>
where
    B: PersistedStore<K>,
    K: PersistedKey,
{
    fn drop(&mut self) {
        let value = self.value.take().unwrap();
        if let Err(_error) =
            B::with_instance(|store| store.set(&self.key, value))
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
        match B::with_instance(|store| store.get(&key)) {
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

/// TODO
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

impl<B, K, C> Drop for PersistedLazy<B, K, C>
where
    B: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value>,
{
    fn drop(&mut self) {
        let value = self.container.get_persisted();
        if let Err(_error) =
            B::with_instance(|store| store.set(&self.key, value))
        {
            // TODO tracing
        }
    }
}

/// TODO
pub trait PersistedKey {
    /// TODO
    type Value;
}

/// TODO
pub trait PersistedContainer {
    type Value;

    /// Get the current value to persist in the store
    fn get_persisted(&self) -> Self::Value;

    /// Set the container's value, based on value loaded from the persistence
    /// store
    fn set_persisted(&mut self, value: Self::Value);
}

/// TODO
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(bound = "K: serde::Serialize"))]
pub struct UniqueKey<'a, K> {
    name: &'static str,
    key: &'a K,
}

impl<'a, K: PersistedKey> UniqueKey<'a, K> {
    /// TODO
    pub fn new(key: &'a K) -> Self {
        Self {
            name: any::type_name::<K>(),
            key,
        }
    }

    /// Get the unique name of the key *type*
    pub fn name(&self) -> &str {
        self.name
    }

    /// Get the contained key
    pub fn key(&self) -> &'a K {
        self.key
    }
}

/// Implement [PersistedKey] for a type, with a fixed value type
/// TODO derive macro
#[macro_export]
macro_rules! impl_persisted_key {
    ($type:ty, $value:ty) => {
        impl $crate::PersistedKey for $type {
            type Value = $value;
        }
    };
}

#[cfg(test)]
mod tests {
    // TODO
}
