# persisted

[![Test CI](https://github.com/github/docs/actions/workflows/test.yml/badge.svg)](https://github.com/LucasPickering/persisted/actions)
[![crates.io](https://img.shields.io/crates/v/persisted.svg)](https://crates.io/crates/persisted)
[![docs.rs](https://img.shields.io/docsrs/persisted)](https://docs.rs/persisted)

`persisted` is a Rust library that makes it easy and quick to save arbitrary program state. Its simple and flexible design means you bring your own data store. You tell `persisted` how to save data and what you want to save, and it figures out the rest.

`persisted` was designed for use in TUI programs (specifically [Slumber](https://github.com/LucasPickering/slumber)), but can also be used for GUIs or any other applications with distributed state that needs to be persisted.

Here’s an example of a very simple persistence scheme. The store keeps just a single value.

```rust
use core::cell::Cell;
use persisted::{Persisted, PersistedKey, PersistedStore};

/// Store index of the selected person
#[derive(Default)]
struct Store(Cell<Option<usize>>);

impl Store {
    thread_local! {
        static INSTANCE: Store = Default::default();
    }
}

impl PersistedStore<SelectedIndexKey> for Store {
    fn load_persisted(_key: &SelectedIndexKey) -> Option<usize> {
        Self::INSTANCE.with(|store| store.0.get())
    }

    fn store_persisted(_key: &SelectedIndexKey, value: &usize) {
        Self::INSTANCE.with(|store| store.0.set(Some(*value)))
    }
}

/// Persist the selected value in the list by storing its index. This is simple
/// but relies on the list keeping the same items, in the same order, between
/// sessions.
#[derive(PersistedKey)]
#[persisted(usize)]
struct SelectedIndexKey;

#[derive(Clone, Debug)]
#[allow(unused)]
struct Person {
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

let list = vec![
    Person {
        name: "Fred".into(),
        age: 17,
    },
    Person {
        name: "Susan".into(),
        age: 29,
    },
    Person {
        name: "Ulysses".into(),
        age: 40,
    },
];

let mut people = SelectList::new(list.clone());
*people.selected_index.get_mut() = 1;
println!("Selected: {}", people.selected().name);
// Selected: Susan

let people = SelectList::new(list);
// The previous value was restored
assert_eq!(*people.selected_index, 1);
println!("Selected: {}", people.selected().name);
// Selected: Susan
```

For more examples, see the `examples/` directory or the [documentation](https://docs.rs/persisted).
