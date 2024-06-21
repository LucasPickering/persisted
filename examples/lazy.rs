//! Lazy persistence allows storing and loading values when some amount of work
//! needs to be done to retrieve and/or restore the value on the data side. In
//! this example, the persisted value is the selected index in a list. Rather
//! than persisting the index directly, we can define `SelectList` as a
//! [PersistedContainer], then persist the entire container. The ID of the
//! selected item the only thing stored in the database; this is merely a
//! convenience to change where in our data tree we declare the persistence.
//!
//! This is useful when you have a generic container (in this case
//! `SelectList`) that may be used multiple times with different persistence
//! keys (or not persisted at all in some cases).

use persisted::{
    PersistedContainer, PersistedKey, PersistedLazy, PersistedStore,
};
use std::cell::Cell;

/// Persist just the stored ID
#[derive(Default)]
struct Store(Cell<Option<PersonId>>);

impl Store {
    thread_local! {
        static INSTANCE: Store = Default::default();
    }
}

impl PersistedStore<SelectedIdKey> for Store {
    fn load_persisted(_key: &SelectedIdKey) -> Option<PersonId> {
        Self::INSTANCE.with(|store| store.0.get())
    }

    fn store_persisted(_key: &SelectedIdKey, value: PersonId) {
        Self::INSTANCE.with(|store| store.0.set(Some(value)))
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct PersonId(u64);

#[derive(Clone, Debug)]
#[allow(unused)]
struct Person {
    id: PersonId,
    name: String,
    age: u32,
}

#[derive(Debug, PersistedKey)]
#[persisted(PersonId)]
struct SelectedIdKey;

/// A list of items, with one item selected
struct SelectList {
    values: Vec<Person>,
    selected_index: usize,
}

impl SelectList {
    fn selected(&self) -> &Person {
        &self.values[self.selected_index]
    }
}

impl PersistedContainer for SelectList {
    type Value = PersonId;

    fn get_persisted(&self) -> Self::Value {
        self.selected().id
    }

    fn set_persisted(&mut self, value: Self::Value) {
        // Find selected person by ID
        self.selected_index = self
            .values
            .iter()
            .enumerate()
            .find(|(_, person)| person.id == value)
            .map(|(i, _)| i)
            .unwrap_or_default();
    }
}

fn main() {
    let person_list = vec![
        Person {
            id: PersonId(23089),
            name: "Fred".into(),
            age: 17,
        },
        Person {
            id: PersonId(28833),
            name: "Susan".into(),
            age: 29,
        },
        Person {
            id: PersonId(93383),
            name: "Ulysses".into(),
            age: 40,
        },
    ];

    let mut people = PersistedLazy::<Store, _, _>::new(
        SelectedIdKey,
        SelectList {
            values: person_list.clone(),
            selected_index: 0,
        },
    );
    people.selected_index = 1;
    println!("Selected: {:?}", people.selected());
    drop(people);

    let people = PersistedLazy::<Store, _, _>::new(
        SelectedIdKey,
        SelectList {
            values: person_list,
            selected_index: 0,
        },
    );
    // The previous value was restored
    assert_eq!(people.selected_index, 1);
    println!("Selected: {:?}", people.selected());
}
