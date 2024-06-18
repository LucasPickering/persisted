//! TODO

use persisted::{
    PersistedContainer, PersistedKey, PersistedLazy, PersistedStore,
};
use rusqlite::{named_params, Connection, OptionalExtension};

/// TODO
/// TODO move into common module
struct Store(Connection);

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
struct SelectList {
    values: Vec<Person>,
    selected_index: usize,
}

#[derive(Debug, PersistedKey)]
#[persisted(PersonId)]
struct SelectedIdKey;

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

impl PersistedStore<SelectedIdKey> for Store {
    fn load_persisted(key: &SelectedIdKey) -> Option<PersonId> {
        let result = Self::INSTANCE.with(|store| {
            store
                .0
                .query_row(
                    "SELECT value FROM persisted WHERE key = :key",
                    named_params! { ":key": SelectedIdKey::type_name() },
                    |row| {
                        let id: u64 = row.get("value")?;
                        Ok(PersonId(id))
                    },
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

    fn store_persisted(key: &SelectedIdKey, value: PersonId) {
        let result = Self::INSTANCE.with(|store| {
            store
                .0
                .execute(
                    // Upsert!
                    "INSERT INTO persisted (key, value)
                    VALUES (:key, :value)
                    ON CONFLICT DO UPDATE SET value = excluded.value",
                    named_params! {
                        ":key": SelectedIdKey::type_name(),
                        ":value": value.0,
                    },
                )
                .map(|_| ())
        });
        if let Err(error) = result {
            println!("Error occured persisting {key:?}={value:?}: {error}");
        }
    }
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
