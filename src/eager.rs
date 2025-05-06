use crate::{PersistedKey, PersistedStore};
use core::{
    fmt::{self, Debug, Display},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// A wrapper that will automatically persist its contained value to the
/// store. The value will be loaded from the store on creation, and saved on
/// mutation.
///
/// ## Generic Params
///
/// - `S`: The backend type used to persist data. While we don't need access to
///   an instance of the backend, we do need to know its type so we can access
///   its static functions on setup/mutation.
/// - `K`: The type of the persistence key. The associated `Value` type will be
///   the type of the contained value.
///
/// ## Accessing
///
/// The inner value can be accessed immutably via [Deref]. To get mutable
/// access, use [Persisted::get_mut]. This wrapper method returns a guard that
/// implements [DerefMut] (similar to `RefMut` or `MutexGuard` from `std`,
/// without the internal mutability). When your mutable access is complete, this
/// wrapper will be dropped and the value, which presumably was changed, will be
/// persisted to the store.
///
/// ## Cloning
///
/// This type intentionally does *not* implement [Clone]. Cloning would result
/// in two values with the same key. When the values are mutated, their
/// persisted values would overwrite each other. It's unlikely this is the
/// desired behavior, and therefore is not provided.
pub struct Persisted<S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey,
{
    backend: PhantomData<S>,
    key: K,
    value: K::Value,
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
            value,
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

    /// Get a reference to this container's key
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Get a mutable reference to the value. This is wrapped by a guard, so
    /// that after mutation when the guard is dropped, the value can be saved.
    pub fn get_mut(&mut self) -> PersistedRefMut<'_, S, K> {
        PersistedRefMut {
            backend: self.backend,
            key: &self.key,
            value: &mut self.value,
        }
    }
}

// Needed to omit Default bound on S
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

// Needed to omit Debug bound on S
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

impl<S, K> Display for Persisted<S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    K::Value: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

impl<S, K> Deref for Persisted<S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey,
{
    type Target = K::Value;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

/// A guard encompassing the lifespan of a mutable reference to a persisted
/// value. The purpose of this is to save the value immediately after it is
/// mutated.
pub struct PersistedRefMut<'a, S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey,
{
    backend: PhantomData<S>,
    key: &'a K,
    value: &'a mut K::Value,
}

// Needed to omit Debug bound on S
impl<'a, S, K> Debug for PersistedRefMut<'a, S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey + Debug,
    K::Value: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PersistedRefMut")
            .field("backend", &self.backend)
            .field("key", &self.key)
            .field("value", &self.value)
            .finish()
    }
}

impl<'a, S, K> Deref for PersistedRefMut<'a, S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey,
{
    type Target = K::Value;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, S, K> DerefMut for PersistedRefMut<'a, S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

/// Save value after modification. This assumes the user modified the value
/// while they had this mutable reference.
impl<'a, S, K> Drop for PersistedRefMut<'a, S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey,
{
    fn drop(&mut self) {
        S::store_persisted(self.key, self.value);
    }
}
