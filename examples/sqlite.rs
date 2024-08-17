//! Persist a simple value via a SQLite database

use persisted::{Persisted, PersistedKey, PersistedStore};
use rusqlite::{named_params, Connection, OptionalExtension};

/// Persist data in a SQLite database
struct Store(Connection);

impl Store {
    const DB_PATH: &'static str = "persisted.sqlite";

    thread_local! {
        static INSTANCE: Store = Store::new();
    }

    fn new() -> Self {
        let connection =
            Connection::open(Self::DB_PATH).expect("Error opening DB");
        connection
            .execute(
                "CREATE TABLE IF NOT EXISTS persisted (
                    key     TEXT NOT NULL,
                    value   INTEGER NOT NULL
                )",
                (),
            )
            .expect("Error initializing table");
        Self(connection)
    }
}

impl PersistedStore<SelectedIndexKey> for Store {
    fn load_persisted(key: &SelectedIndexKey) -> Option<usize> {
        let result = Self::INSTANCE.with(|store| {
            store
                .0
                .query_row(
                    "SELECT value FROM persisted WHERE key = :key",
                    named_params! { ":key": SelectedIndexKey::type_name() },
                    |row| row.get("value"),
                )
                .optional()
        });
        match result {
            Ok(option) => option,
            // You can replace this with logging, tracing, etc.
            Err(error) => {
                println!(
                    "Error occured loading value for key {key:?}: {error}"
                );
                None
            }
        }
    }

    fn store_persisted(key: &SelectedIndexKey, value: &usize) {
        let result = Self::INSTANCE.with(|store| {
            store
                .0
                .execute(
                    // Upsert!
                    "INSERT INTO persisted (key, value)
                    VALUES (:key, :value)
                    ON CONFLICT DO UPDATE SET value = excluded.value",
                    named_params! {
                        ":key": SelectedIndexKey::type_name(),
                        ":value": value,
                    },
                )
                .map(|_| ())
        });
        if let Err(error) = result {
            println!("Error occured persisting {key:?}={value:?}: {error}");
        }
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
#[derive(Debug, PersistedKey)]
#[persisted(usize)]
struct SelectedIndexKey;

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

    let mut people = SelectList::new(person_list.clone());
    *people.selected_index.get_mut() = 1;
    println!("Selected: {:?}", people.selected());
    drop(people);

    let people = SelectList::new(person_list);
    // The previous value was restored
    assert_eq!(*people.selected_index, 1);
    println!("Selected: {:?}", people.selected());
}
