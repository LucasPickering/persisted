use crate::{PersistedContainer, PersistedKey, PersistedStore};
use core::marker::PhantomData;
use derive_more::{Deref, DerefMut, Display};

/// Similar to [Persisted](crate::eager::Persisted), but the value that's sent
/// to the store is not the same as the value stored in memory. Instead, the
/// value is computed at save time by [PersistedContainer::get_persisted].
/// Similarly, the persisted value that's loaded at initialization isn't stored
/// directly in the container. Instead, [PersistedContainer::set_persisted]
/// determines how to initialize state based on it.
///
/// This is useful if the value you want to store is some derivation of the
/// value you keep in memory. For example, storing which item in a list is
/// selected: if you store the index of the selected item in memory but want to
/// persist the *ID* of the selected item so it's resilient to re-ordering, you
/// can use this.
///
/// **Note:** Unlike `Persisted`, the value of `PersistedLazy` is only persisted
/// on **drop**, not whenever it is mutated. This is a technical limitation that
/// will hopefully be fixed in the future.
///
/// ## Generic Params
///
/// - `S`: The backend type used to persist data. While we don't need access to
///   an instance of the backend, we do need to know its type so we can access
///   its static functions on setup/drop.
/// - `K`: The type of the persistence key
/// - `C`: The type of the wrapping container (see [PersistedContainer]). The
///   type of the container's persisted value must match the expected value for
///   the key. In other words, `K::Value` must equal `C::Value`.
///
/// ## Accessing
///
/// The inner container is accessed and modified transparently via [Deref] and
/// [DerefMut] implementations.
///
/// ## Cloning
///
/// This type intentionally does *not* implement [Clone]. Cloning would result
/// in two containers with the same key. When the containers are eventually
/// dropped, whichever is dropped first would have its persisted value
/// overwritten by the other. It's unlikely this is the desired behavior, and
/// therefore is not provided.
///
/// ## Example
///
/// ```
/// use persisted::{
///     PersistedContainer, PersistedKey, PersistedLazy, PersistedStore,
/// };
/// use std::cell::Cell;
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
///     fn get_persisted(&self) -> Self::Value {
///         self.selected().id
///     }
///
///     fn set_persisted(&mut self, value: Self::Value) {
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
/// people.selected_index = 1;
/// assert_eq!(people.selected().id.0, 28833);
/// drop(people);
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
#[derive(derive_more::Debug, Deref, DerefMut, Display)]
#[display("{}", container)]
pub struct PersistedLazy<S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value>,
{
    #[debug(skip)] // Omit bound on S
    backend: PhantomData<S>,
    key: K,
    #[deref]
    #[deref_mut]
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
    /// [PersistedContainer::set_persisted].
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

    /// Initialize a new default container whose value will lazily be loaded and
    /// persisted. If a persisted value is available in the store, it will be
    /// loaded and used to initialize the container via
    /// [PersistedContainer::set_persisted].
    pub fn new_default(key: K) -> Self
    where
        C: Default,
    {
        Self::new(key, C::default())
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

/// Save value after modification. This assumes the user modified the value
/// while they had this mutable reference.
impl<S, K, C> Drop for PersistedLazy<S, K, C>
where
    S: PersistedStore<K>,
    K: PersistedKey,
    C: PersistedContainer<Value = K::Value>,
{
    fn drop(&mut self) {
        let value = self.container.get_persisted();
        S::store_persisted(&self.key, &value);
    }
}
