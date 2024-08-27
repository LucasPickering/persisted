# Changelog

All user-facing changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] - ReleaseDate

## [0.3.1] - 2024-08-27

### Fixed

- Don't save `PersistedLazy` contents on drop
  - This was a bug, lingering from pre-0.3

## [0.3.0] - 2024-08-17

### Breaking

- Rename `Persisted::borrow_mut` to `get_mut`
- Rename `PersistedContainer::get_persisted` to `PersistedContainer::get_to_persisted`
- Rename `PersistedContainer::set_persisted` to `PersistedContainer::restore_persisted`
- Persist values on mutation rather than drop for `PersistedLazy`
  - Similar to `Persisted`, `PersistedLazy` now has a `get_mut` method that returns a ref guard

## [0.2.2] - 2024-08-09

### Changed

- Upgrade `derive_more` to 1.0.0

## [0.2.1] - 2024-08-02

### Fixed

- Fix broken internal link in docs

## [0.2.0] - 2024-08-02

### Breaking

- Exclude `phantom` field from `SingletonKey` during serialization
- Persist values on mutation rather than just on drop, for `Persisted` only

## [0.1.1] - 2024-06-21

## [0.1.0] - 2024-06-21

### Added

- Initial functionality
