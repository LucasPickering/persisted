//! Persist a simple value via SQLite

use persisted::{Persisted, PersistedKey, PersistedStore};
use rusqlite::{named_params, Connection, OptionalExtension};

/// TODO
struct Store(Connection);

#[derive(Copy, Clone, Debug, PartialEq)]
struct PersonId(u64);

/// TODO
#[derive(Clone, Debug)]
#[allow(unused)]
struct Person {
    id: PersonId,
    name: String,
    age: u32,
}

/// TODO
struct SelectList<T> {
    values: Vec<T>,
    selected_index: Persisted<Store, SelectedIndexKey>,
}

/// Persist the selected value in the list by storing its index. This is simple
/// but relies on the list keeping the same items, in the same order, between
/// sessions.
struct SelectedIndexKey;

impl SelectedIndexKey {
    const DB_KEY: &'static str = "selected_index";
}

impl PersistedKey for SelectedIndexKey {
    type Value = usize;
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

    let mut people = SelectList::new(person_list.clone());
    *people.selected_index = 1;
    println!("Selected: {:?}", people.selected());
    drop(people);

    let people = SelectList::new(person_list);
    // The previous value was restored
    assert_eq!(*people.selected_index, 1);
    println!("Selected: {:?}", people.selected());
}

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
    type Error = rusqlite::Error;

    fn with_instance<T>(f: impl FnOnce(&Self) -> T) -> T {
        Self::INSTANCE.with(f)
    }

    fn get(
        &self,
        _key: &SelectedIndexKey,
    ) -> Result<Option<usize>, Self::Error> {
        self.0
            .query_row(
                "SELECT value FROM persisted WHERE key = :key",
                named_params! { ":key": SelectedIndexKey::DB_KEY },
                |row| row.get("value"),
            )
            .optional()
    }

    fn set(
        &self,
        _key: &SelectedIndexKey,
        index: usize,
    ) -> Result<(), Self::Error> {
        self.0
            .execute(
                // Upsert!
                "INSERT INTO persisted (key, value)
                VALUES (:key, :value)
                ON CONFLICT DO UPDATE SET value = excluded.value",
                named_params! {
                    ":key": SelectedIndexKey::DB_KEY,
                    ":value": index,
                },
            )
            .map(|_| ())
    }
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
