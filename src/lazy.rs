use crate::{PersistedKey, PersistedStore};
use core::{
    fmt::{self, Debug, Display},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// Similar to [Persisted](crate::eager::Persisted), but the value that's sent
/// to the store is not the same as the value stored in memory. Instead, the
/// value is computed at save time by [PersistedContainer::get_to_persist].
/// Similarly, the persisted value that's loaded at initialization isn't stored
/// directly in the container. Instead, [PersistedContainer::restore_persisted]
/// determines how to initialize state based on it.
///
/// This is useful if the value you want to store is some derivation of the
/// value you keep in memory. For example, storing which item in a list is
/// selected: if you store the index of the selected item in memory but want to
/// persist the *ID* of the selected item so it's resilient to re-ordering, you
/// can use this.
///
/// ## Generic Params
///
/// - `S`: The backend type used to persist data. While we don't need access to
///   an instance of the backend, we do need to know its type so we can access
///   its static functions on setup/save.
/// - `K`: The type of the persistence key
/// - `C`: The type of the wrapping container (see [PersistedContainer]). The
///   type of the container's persisted value must match the expected value for
///   the key. In other words, `K::Value` must equal `C::Value`.
///
/// ## Accessing
///
/// The inner value can be accessed immutably via [Deref]. To get mutable
/// access, use [PersistedLazy::get_mut]. This wrapper method returns a guard
/// that implements [DerefMut] (similar to `RefMut` or `MutexGuard` from `std`,
/// without the internal mutability). When your mutable access is complete, this
/// wrapper will be dropped and the value will be persisted to the store **only
/// if it changed** (according to its [PartialEq] impl).
///
/// ## Cloning
///
/// This type intentionally does *not* implement [Clone]. Cloning would result
/// in two containers with the same key. Whenever a modification is made to one
/// it will overwrite the persistence slot. It's unlikely this is the desired
/// behavior, and therefore is not provided.
///
/// ## Example
///
/// ```
/// use persisted::{
///     PersistedContainer, PersistedKey, PersistedLazy, PersistedStore,
/// };
/// use core::cell::Cell;
///
/// /// Persist just the stored ID
/// #[derive(Default)]
/// struct Store(Cell<Option<PersonId>>);
///
/// impl Store {
///     thread_local! {
///         static INSTANCE: Store = Default::default();
///     }
/// }
///
/// impl PersistedStore<SelectedIdKey> for Store {
///     fn load_persisted(_key: &SelectedIdKey) -> Option<PersonId> {
///         Self::INSTANCE.with(|store| store.0.get())
///     }
///
///     fn store_persisted(_key: &SelectedIdKey, value: &PersonId) {
///         Self::INSTANCE.with(|store| store.0.set(Some(*value)))
///     }
/// }
///
/// #[derive(Copy, Clone, Debug, PartialEq)]
/// struct PersonId(u64);
///
/// #[derive(Clone, Debug)]
/// #[allow(unused)]
/// struct Person {
///     id: PersonId,
///     name: String,
///     age: u32,
/// }
///
/// #[derive(Debug, PersistedKey)]
/// #[persisted(PersonId)]
/// struct SelectedIdKey;
///
/// /// A list of items, with one item selected
/// struct SelectList {
///     values: Vec<Person>,
///     selected_index: usize,
/// }
///
/// impl SelectList {
///     fn selected(&self) -> &Person {
///         &self.values[self.selected_index]
///     }
/// }
///
/// impl PersistedContainer for SelectList {
///     type Value = PersonId;
///
///     fn get_to_persist(&self) -> Self::Value {
///         self.selected().id
///     }
///
///     fn restore_persisted(&mut self, value: Self::Value) {
///         // Find selected person by ID
///         self.selected_index = self
///             .values
///             .iter()
///             .enumerate()
///             .find(|(_, person)| person.id == value)
///             .map(|(i, _)| i)
///             .unwrap_or_default();
///     }
/// }
///
/// let person_list = vec![
///     Person {
///         id: PersonId(23089),
///         name: "Fred".into(),
///         age: 17,
///     },
///     Person {
///         id: PersonId(28833),
///         name: "Susan".into(),
///         age: 29,
///     },
///     Person {
///         id: PersonId(93383),
///         name: "Ulysses".into(),
///         age: 40,
///     },
/// ];
///
/// let mut people = PersistedLazy::<Store, _, _>::new(
///     SelectedIdKey,
///     SelectList {
///         values: person_list.clone(),
///         selected_index: 0,
///     },
/// );
/// people.get_mut().selected_index = 1;
/// assert_eq!(people.selected().id.0, 28833);
///
/// let people = PersistedLazy::<Store, _, _>::new(
///     SelectedIdKey,
///     SelectList {
///         values: person_list,
///         selected_index: 0,
///     },
/// );
/// // The previous value was restored
/// assert_eq!(people.selected_index, 1);
/// assert_eq!(people.selected().id.0, 28833);
/// ```
pub struct PersistedLazy<S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value>,
{
    backend: PhantomData<S>,
    key: K,
    /// Cache the most recently persisted value so we can check if it's changed
    /// after each mutable access. When it does change, we'll persist.
    last_persisted: Option<K::Value>,
    container: C,
}

impl<S, K, C> PersistedLazy<S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value>,
{
    /// Initialize a given container whose value will lazily be loaded and
    /// persisted. If a persisted value is available in the store, it will be
    /// loaded and used to initialize the container via
    /// [PersistedContainer::restore_persisted].
    pub fn new(key: K, mut container: C) -> Self {
        // Fetch persisted value from the backend
        if let Some(value) = S::load_persisted(&key) {
            container.restore_persisted(value);
        }

        Self {
            backend: PhantomData,
            key,
            container,
            last_persisted: None,
        }
    }

    /// Initialize a new default container whose value will lazily be loaded and
    /// persisted. If a persisted value is available in the store, it will be
    /// loaded and used to initialize the container via
    /// [PersistedContainer::restore_persisted].
    pub fn new_default(key: K) -> Self
    where
        C: Default,
    {
        Self::new(key, C::default())
    }

    /// Get a reference to this container's key
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Get a mutable reference to the value. This is wrapped by a guard, so
    /// that after mutation when the guard is dropped, the value can be
    /// persisted. [PersistedStore::store_persisted] will only be called if the
    /// persisted value actually changed, hence the `K::Value: PartialEq` bound.
    /// This means [PersistedContainer::get_to_persist] will be called after
    /// event mutable access, but the value will only be written to the store
    /// when it's been modified.
    pub fn get_mut(&mut self) -> PersistedLazyRefMut<S, K, C>
    where
        K::Value: PartialEq,
    {
        PersistedLazyRefMut { lazy: self }
    }
}

// Needed to omit Default bound on S
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

// Needed to omit Debug bound on S
impl<S, K, C> Debug for PersistedLazy<S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey + Debug,
    K::Value: Debug,
    C: PersistedContainer<Value = K::Value> + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PersistedLazy")
            .field("backend", &self.backend)
            .field("key", &self.key)
            .field("last_persisted", &self.last_persisted)
            .field("container", &self.container)
            .finish()
    }
}

impl<S, K, C> Display for PersistedLazy<S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value> + Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.container.fmt(f)
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

/// A guard encompassing the lifespan of a mutable reference to a lazy
/// container. The purpose of this is to save the value immediately after it is
/// mutated. **The save will only occur if the value actually changed.** A copy
/// of the previous value is saved before the mutable access, and compared after
/// the access.
pub struct PersistedLazyRefMut<'a, S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    K::Value: PartialEq,
    C: PersistedContainer<Value = K::Value>,
{
    lazy: &'a mut PersistedLazy<S, K, C>,
}

// Needed to omit Debug bound on S
impl<'a, S, K, C> Debug for PersistedLazyRefMut<'a, S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey + Debug,
    K::Value: PartialEq + Debug,
    C: PersistedContainer<Value = K::Value> + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PersistedLazyRefMut")
            .field("lazy", &self.lazy)
            .finish()
    }
}

impl<'a, S, K, C> Deref for PersistedLazyRefMut<'a, S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    K::Value: PartialEq,
    C: PersistedContainer<Value = K::Value>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.lazy.container
    }
}

impl<'a, S, K, C> DerefMut for PersistedLazyRefMut<'a, S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    K::Value: PartialEq,
    C: PersistedContainer<Value = K::Value>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.lazy.container
    }
}

/// Save value after modification **only if it changed**
impl<'a, S, K, C> Drop for PersistedLazyRefMut<'a, S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    K::Value: PartialEq,
    C: PersistedContainer<Value = K::Value>,
{
    fn drop(&mut self) {
        let persisted_value = self.lazy.container.get_to_persist();
        if !self
            .lazy
            .last_persisted
            .as_ref()
            .is_some_and(|last_persisted| last_persisted == &persisted_value)
        {
            S::store_persisted(&self.lazy.key, &persisted_value);
            self.lazy.last_persisted = Some(persisted_value);
        }
    }
}

/// A container that can store and provide a persisted value. This is used in
/// conjunction with [PersistedLazy] to define how to lazily get the value that
/// should be persisted, and how to restore state when a persisted value is
/// loaded during initialization.
pub trait PersistedContainer {
    /// The value to be persisted
    type Value;

    /// Get the current value to persist in the store
    fn get_to_persist(&self) -> Self::Value;

    /// Set the container's value, based on value loaded from the store
    fn restore_persisted(&mut self, value: Self::Value);
}
