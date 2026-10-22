# Spec Delta

## ADDED Requirements

### Requirement: Binding-free core

Binding attributes (UniFFI, wasm-bindgen) SHALL appear only in the façade crates `guardedsleep-ffi` and `guardedsleep-wasm`, never in `guardedsleep-core`, and TypeScript protocol types SHALL be generated from the core.

#### Scenario: UniFFI attribute in the core

- **WHEN** a change adds a UniFFI attribute to `guardedsleep-core`
- **THEN** the build or lint fails

### Requirement: API-surface snapshots

The public API of `guardedsleep-ffi` and `guardedsleep-wasm` and the generated `.d.ts` SHALL be committed as snapshots, and CI SHALL fail when they differ from the code. CI SHALL generate the bindings twice and fail when the hashes differ.

#### Scenario: Unnoticed API change

- **WHEN** a change renames a function exported to Kotlin without updating the snapshot
- **THEN** CI fails and shows the API diff

### Requirement: Vectors through every binding

Conformance vectors, including negative vectors, SHALL run through the Rust core, the Kotlin bindings and the TypeScript bindings.

#### Scenario: Binding-specific parsing bug

- **WHEN** the Kotlin binding accepts a vector that the core rejects
- **THEN** the Kotlin vector test fails
