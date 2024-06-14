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

/// TODO
#[derive(Clone, Debug)]
#[allow(unused)]
struct Person {
    id: PersonId,
    name: String,
    age: u32,
}

/// TODO
struct SelectList {
    values: Vec<Person>,
    selected_index: usize,
}

struct SelectedIdKey;

impl SelectedIdKey {
    const DB_KEY: &'static str = "selected_id";
}

impl PersistedKey for SelectedIdKey {
    type Value = PersonId;
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
            values: person_list.clone(),
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
    type Error = rusqlite::Error;

    fn with_instance<T>(f: impl FnOnce(&Self) -> T) -> T {
        Self::INSTANCE.with(f)
    }

    fn get(
        &self,
        _key: &SelectedIdKey,
    ) -> Result<Option<PersonId>, Self::Error> {
        self.0
            .query_row(
                "SELECT value FROM persisted WHERE key = :key",
                named_params! { ":key": SelectedIdKey::DB_KEY },
                |row| {
                    let id: u64 = row.get("value")?;
                    Ok(PersonId(id))
                },
            )
            .optional()
    }

    fn set(
        &self,
        _key: &SelectedIdKey,
        value: PersonId,
    ) -> Result<(), Self::Error> {
        self.0
            .execute(
                // Upsert!
                "INSERT INTO persisted (key, value)
                VALUES (:key, :value)
                ON CONFLICT DO UPDATE SET value = excluded.value",
                named_params! {
                    ":key": SelectedIdKey::DB_KEY,
                    ":value": value.0,
                },
            )
            .map(|_| ())
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