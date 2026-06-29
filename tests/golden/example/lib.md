# Crate `example`

_Defined at_ `src/lib.rs:1`

Example crate used as a documentation fixture.

Mirrors the module layout described in `GOALS.md`, and exercises the
renderer across item kinds.

## Modules

- [`top`](top/mod.md)

## Types

- [`Alpha`](Alpha.md) — First item exposed via a glob reexport.
- [`Beta`](Beta.md) — Second item exposed via a glob reexport.
- [`Color`](Color.md) — An RGB or named color.
- [`Foo`](Foo.md) — A crate-root type.
- [`Point`](Point.md) — A point with named fields.
- [`Reexported`](Reexported.md) — Reachable only via the reexport below.
- [`Renamed`](Renamed.md) — Defined privately, exposed under a different public name.

## Traits

- [`Shape`](Shape.md) — A geometric shape.

## Functions

### `greet`

```rust
pub fn greet<S: Into<String>>(name: S) -> String
```

Greet by name, returning the greeting.

The greeting has nothing to do with a [`Point`], but this links to one to
exercise intra-doc link resolution.

[`Point`]: Point.md

## Constants

### `MAX_DEPTH`

```rust
pub const MAX_DEPTH: u32
```

A configuration value.

## Type Aliases

### `Pair`

```rust
pub type Pair = (u32, u32);
```

A convenient alias.
