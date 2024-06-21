//! Persist values into a hashmap. For most use cases this is not practical
//! because the hashmap will be lost on program shutdown anyway. However, this
//! pattern is effective if you want to persist values between multiple life
//! cycles of some subsection of your program, within the span of one process.

use persisted::{Persisted, PersistedKey, PersistedStore};
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::{self, Debug, Display},
    str::FromStr,
};

/// The key is a pair of the key's type and content. The value is a stringified
/// version of the value. Typically you would want to replacing stringification
/// and parsing with a more robust form of serialization/deserialization, but
/// this example is simplified to not rely on dependencies.
///
/// We need a `RefCell` to allow mutable access to the store from the drop
/// handler of persisted values.
#[derive(Default)]
struct Store(RefCell<HashMap<(&'static str, String), String>>);

impl Store {
    thread_local! {
        static INSTANCE: Store = Default::default();
    }
}

impl<K> PersistedStore<K> for Store
where
    K: Display + PersistedKey,
    K::Value: Display + FromStr,
    <K::Value as FromStr>::Err: Debug,
{
    fn load_persisted(key: &K) -> Option<K::Value> {
        Self::INSTANCE.with(|store| {
            let map = store.0.borrow();
            let value_str = map.get(&(K::type_name(), key.to_string()));
            value_str.map(|value| value.parse().expect("Error parsing value"))
        })
    }

    fn store_persisted(key: &K, value: K::Value) {
        Self::INSTANCE.with(|store| {
            let mut map = store.0.borrow_mut();
            map.insert((K::type_name(), key.to_string()), value.to_string());
        })
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct PersonId(u64);

#[derive(Debug)]
#[allow(unused)]
struct Person {
    id: PersonId,
    name: String,
    age: u32,
    enabled: Persisted<Store, ToggleKey>,
}

impl Person {
    fn new(id: PersonId, name: String, age: u32) -> Self {
        let enabled = Persisted::new(ToggleKey(id), true);
        Self {
            id,
            name,
            age,
            enabled,
        }
    }
}

/// A list of items, with one item selected
struct SelectList<T> {
    values: Vec<T>,
    selected_index: Persisted<Store, SelectedIndexKey>,
}

impl<T> SelectList<T> {
    fn new(values: Vec<T>) -> Self {
        Self {
            values,
            selected_index: Persisted::new(SelectedIndexKey, 0),
        }
    }

    fn selected(&self) -> &T {
        &self.values[*self.selected_index]
    }
}

/// Persist the selected value in the list by storing its index. This is simple
/// but relies on the list keeping the same items, in the same order, between
/// sessions.
#[derive(PersistedKey)]
#[persisted(usize)]
struct SelectedIndexKey;

impl Display for SelectedIndexKey {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Nothing to include here, so serialize as the empty string
        Ok(())
    }
}

#[derive(Debug, PersistedKey)]
#[persisted(bool)]
struct ToggleKey(PersonId);

impl Display for ToggleKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", (self.0).0)
    }
}

fn main() {
    let make_list = || {
        vec![
            Person::new(PersonId(23089), "Fred".into(), 17),
            Person::new(PersonId(28833), "Susan".into(), 29),
            Person::new(PersonId(93383), "Ulysses".into(), 40),
        ]
    };

    let mut people = SelectList::new(make_list());
    *people.selected_index = 1;
    *people.values[1].enabled = false;
    println!("Selected: {:?}", people.selected());
    drop(people);

    let people = SelectList::new(make_list());
    // The previous values were restored
    assert_eq!(*people.selected_index, 1);
    assert!(!*people.values[1].enabled);
    println!("Selected: {:?}", people.selected());
}
