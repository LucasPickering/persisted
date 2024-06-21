#![no_std]
// TODO integration tests

//! `persisted` is a library for persisting arbitrary values in your program so
//! they can easily be restored later. The main goals of the library are:
//!
//! - Explicitness: You define exactly what is persisted, and its types
//! - Ease of use: Thin wrappers make persisting values easy
//! - Flexible: `persisted` is data store-agnostic; use any persistence scheme
//!   you want, including a database, key-value store, etc.
//!
//! `persisted` was designed originally for use in
//! [Slumber](https://crates.io/crates/slumber), a TUI HTTP client. As such, its
//! main use case is for persisting values between sessions in a user interface.
//! It is very flexible though, and could be used for persisting any type of
//! value in any type of context. `no_std` support means it can even be used in
//! embedded contexts.
//!
//! ## Concepts
//!
//! `persisted` serves as a middleman between your values and your data store.
//! You define your data structures and how your data should be saved,
//! and `persisted` makes sure the data is loaded and stored appropriately. The
//! key concepts are:
//!
//! - Data wrappers: [Persisted] and [PersistedLazy]
//!     - These wrap your data to automatically restore and save values from/to
//!       the store
//! - Data store: any implementor of [PersistedStore]
//! - Key: A unique identifier for a value in the store. Each persisted value
//!   must have its own key. Key types must implement [PersistedKey].
//!
//! ## How Does It Work?
//!
//! `persisted` works by wrapping each persisted value in either [Persisted] or
//! [PersistedLazy]. The wrapper is created with a key and optionally a default
//! value. A request is made to the store to load the most recent value for the
//! key, and if present that value is used. When the wrapper is dropped, the
//! a request is made to the store to save the final value.
//!
//! Because the store is accessed from constructors and destructors, it cannot
//! be passed around and must be reachable statically. The easiest way to do
//! this is with either a `static` or `thread_local` definition of your store.
//!
//! ## Example
//!
//! Here's an example of a very simple persistence scheme. The store keeps just
//! a single value.
//!
//! ```
//! use core::cell::Cell;
//! use persisted::{Persisted, PersistedKey, PersistedStore};
//!
//! /// Store index of the selected person
//! #[derive(Default)]
//! struct Store(Cell<Option<usize>>);
//!
//! impl Store {
//!     thread_local! {
//!         static INSTANCE: Store = Default::default();
//!     }
//! }
//!
//! impl PersistedStore<SelectedIndexKey> for Store {
//!     fn load_persisted(_key: &SelectedIndexKey) -> Option<usize> {
//!         Self::INSTANCE.with(|store| store.0.get())
//!     }
//!
//!     fn store_persisted(_key: &SelectedIndexKey, value: usize) {
//!         Self::INSTANCE.with(|store| store.0.set(Some(value)))
//!     }
//! }
//!
//! /// Persist the selected value in the list by storing its index. This is simple
//! /// but relies on the list keeping the same items, in the same order, between
//! /// sessions.
//! #[derive(PersistedKey)]
//! #[persisted(usize)]
//! struct SelectedIndexKey;
//!
//! #[derive(Clone, Debug)]
//! #[allow(unused)]
//! struct Person {
//!     name: String,
//!     age: u32,
//! }
//!
//! /// A list of items, with one item selected
//! struct SelectList<T> {
//!     values: Vec<T>,
//!     selected_index: Persisted<Store, SelectedIndexKey>,
//! }
//!
//! impl<T> SelectList<T> {
//!     fn new(values: Vec<T>) -> Self {
//!         Self {
//!             values,
//!             selected_index: Persisted::new(SelectedIndexKey, 0),
//!         }
//!     }
//!
//!     fn selected(&self) -> &T {
//!         &self.values[*self.selected_index]
//!     }
//! }
//!
//! let list = vec![
//!     Person {
//!         name: "Fred".into(),
//!         age: 17,
//!     },
//!     Person {
//!         name: "Susan".into(),
//!         age: 29,
//!     },
//!     Person {
//!         name: "Ulysses".into(),
//!         age: 40,
//!     },
//! ];
//!
//! let mut people = SelectList::new(list.clone());
//! *people.selected_index = 1;
//! println!("Selected: {}", people.selected().name);
//! // Selected: Susan
//! drop(people);
//!
//! let people = SelectList::new(list);
//! // The previous value was restored
//! assert_eq!(*people.selected_index, 1);
//! println!("Selected: {}", people.selected().name);
//! // Selected: Susan
//! ```

/// Derive macro for [PersistedKey]
#[cfg(feature = "derive")]
pub use persisted_derive::PersistedKey;

use core::{
    any,
    fmt::{self, Debug},
    marker::PhantomData,
};
use derive_more::{Deref, DerefMut, Display};

/// A trait for any data store capable of persisting data. A store is the layer
/// that saves data. It could save it in memory, on disk, over the network, etc.
/// The generic parameter `K` defines which keys this store is capable of
/// saving. For example, if your storage mechanism involves stringifyin keys,
/// your implementation may look like:
///
/// ```ignore
/// struct Store;
///
/// impl<K: PersistedKey + Display> for Store {
///     ...
/// }
/// ```
///
/// This trait enforces three key requirements on all implementors:
///
/// - It is statically accessible, i.e. you can load and store data without a
///   reference to the store
/// - It does not return errors. This does **not** mean it is infallible; it
///   just means that if errors can occur during load/store, they are handled
///   within the implementation rather than propagated. For example, they could
///   be logged for debugging.
/// - Its is synchronous. `async` load and store operations are not supported.
///
/// All three of these requirements derive from how the store is accessed.
/// Values are loaded during initialization by [Persisted]/[PersistedLazy], and
/// saved by their respective [Drop] implementations. In both cases, there is no
/// reference to the store available, and no way of propagating errors or
/// futures. For this reason, your store access should be _fast_, to prevent
/// latency in your program.
pub trait PersistedStore<K: PersistedKey> {
    /// Load a persisted value from the store, identified by the given key.
    /// Return `Ok(None)` if the value isn't present.
    fn load_persisted(key: &K) -> Option<K::Value>;

    /// Persist a value in the store, under the given key
    fn store_persisted(key: &K, value: K::Value);
}

/// A wrapper that will automatically persist its contained value to the
/// store. The value will be loaded from the store on creation, and saved on
/// drop.
///
/// ## Generic Params
///
/// - `S`: The backend type used to persist data. While we don't need access to
///   an instance of the backend, we do need to know its type so we can access
///   its static functions on setup/drop.
/// - `K`: The type of the persistence key. The associated `Value` type will be
///   the type of the contained value.
///
/// ## Accessing
///
/// The inner value is accessed and modified transparently via [Deref] and
/// [DerefMut] implementations
///
/// ## Cloning
///
/// This type intentionally does *not* implement [Clone]. Cloning would result
/// in two values with the same key. When the values are eventually dropped,
/// whichever is dropped first would have its persisted value overwritten by
/// the other. It's unlikely this is the desired behavior, and therefore is not
/// provided.
#[derive(derive_more::Debug, Display)]
#[display(bound(K::Value: Display))]
#[display("{}", value.as_ref().unwrap())] // See invariant on field
pub struct Persisted<S, K>
where
    S: PersistedStore<K>,
    K: PersistedKey,
{
    #[debug(skip)] // Omit bound on S
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

/// Similar to [Persisted], but the value that's sent to the store is not the
/// same as the value stored in memory. Instead, the value is computed at save
/// time by [PersistedContainer::get_persisted]. Similarly, the persisted value
/// that's loaded at initialization isn't stored directly in the container.
/// Instead, [PersistedContainer::set_persisted] determines how to initialize
/// state based on it.
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
///     fn store_persisted(_key: &SelectedIdKey, value: PersonId) {
///         Self::INSTANCE.with(|store| store.0.set(Some(value)))
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

/// A unique key mapped to a persisted state value in your program. A key can
/// be any Rust value. Unit keys are useful for top-level fields that appear
/// only once in state. Keys can also carry additional data, such as an index or
/// identifier.
///
/// It's very uncommon that you need to implement this trait yourself. In most
/// cases you can use the derive macro (requires `derive` feature to be
/// enabled).
///
/// Regardless of the structure of your keys, you should ensure that each key
/// (not key *type*) appears only once in your state. More formally, for all
/// keys in your state, `key1 != key2`. If two identical keys exist, they will
/// conflict with each other for the same storage slot in the persistence store.
///
/// Some examples of keys:
///
/// ```
/// use persisted::PersistedKey;
///
/// #[derive(PersistedKey)]
/// #[persisted(u64)]
/// struct SelectedFrobnicatorKey;
///
/// #[derive(PersistedKey)]
/// #[persisted(bool)]
/// struct FrobnicatorEnabled(u64);
///
/// #[derive(PersistedKey)]
/// #[persisted(bool)]
/// enum FrobnicatorComponentEnabled {
///     Component1,
///     Component2,
/// }
/// ```
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
    /// [core::any::type_name]. However, for wrapper key types (e.g.
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

/// A persisted key for a value type that appears only once in a program. The
/// **name of the value type** is the only information available as the key,
/// hence why the value type must only be used once.
///
/// ## Example
///
/// ```
/// use persisted::{Persisted, PersistedKey, PersistedStore, SingletonKey};
///
/// enum Foo {
///     Bar,
///     Baz,
/// }
///
/// #[derive(PersistedKey)]
/// #[persisted(Foo)]
/// struct FooKey;
///
/// // These two values are equivalent
/// let value1: Persisted<Store, _> =
///     Persisted::new(SingletonKey::default(), Foo::Bar);
/// let value2: Persisted<Store, _> = Persisted::new(FooKey, Foo::Bar);
///
///
/// struct Store;
///
/// impl<K: PersistedKey> PersistedStore<K> for Store {
///     fn load_persisted(key: &K) -> Option<K::Value> {
///         None
///     }
///     fn store_persisted(key: &K, value: K::Value) {}
/// }
/// ```
#[derive(Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SingletonKey<V> {
    phantom: PhantomData<V>,
}

impl<V> PersistedKey for SingletonKey<V> {
    type Value = V;

    fn type_name() -> &'static str {
        // If the key is wrapped in a container type like Option, this *will*
        // include all the generic params, so it remains unique
        any::type_name::<V>()
    }
}

// Needed to omit Debug bound on V
impl<V> Debug for SingletonKey<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SingletonKey")
            .field("phantom", &self.phantom)
            .finish()
    }
}

// Needed to omit Default bound on V
impl<V> Default for SingletonKey<V> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_singleton_key() {
        struct Foo;

        assert_eq!(
            SingletonKey::<Foo>::type_name(),
            "persisted::tests::test_singleton_key::Foo"
        );
        // Wrapped types also work
        assert_eq!(
            SingletonKey::<Option<Foo>>::type_name(),
            "core::option::Option<persisted::tests::test_singleton_key::Foo>"
        )
    }
}
