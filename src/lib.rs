#![no_std]

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
//! key, and if present that value is used. Whenever the value is modified, the
//! store is notified of the new value so it can be saved.
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
//!     fn store_persisted(_key: &SelectedIndexKey, value: &usize) {
//!         Self::INSTANCE.with(|store| store.0.set(Some(*value)))
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
//! *people.selected_index.get_mut() = 1;
//! println!("Selected: {}", people.selected().name);
//! // Selected: Susan
//!
//! let people = SelectList::new(list);
//! // The previous value was restored
//! assert_eq!(*people.selected_index, 1);
//! println!("Selected: {}", people.selected().name);
//! // Selected: Susan
//! ```
//!
//! ### Feature Flags
//!
//! `persisted` supports the following Cargo features:
//! - `derive` (default): Enable derive macros
//! - `serde`: Enable `Serialize/Deserialize` implementations

mod eager;
mod lazy;

pub use crate::{eager::Persisted, lazy::PersistedLazy};
/// Derive macro for [PersistedKey]
#[cfg(feature = "derive")]
pub use persisted_derive::PersistedKey;

use core::{
    any,
    fmt::{self, Debug},
    marker::PhantomData,
};

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
    fn store_persisted(key: &K, value: &K::Value);
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
///     fn store_persisted(key: &K, value: &K::Value) {}
/// }
/// ```
#[derive(Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SingletonKey<V> {
    #[cfg_attr(feature = "serde", serde(skip))]
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
