# Struct `example::Point`

_Defined at_ `src/lib.rs:26`

A point with named fields.

A `Point` implements [`Shape`]. See also [`greet`].

[`Shape`]: Shape.md
[`greet`]: lib.md#greet

## Definition

```rust
pub struct Point {
    /// The horizontal coordinate.
    pub x: f64,
    /// The vertical coordinate.
    pub y: f64,
}
```

## Implementations

### `impl Point`

#### `magnitude`

```rust
pub fn magnitude(&self) -> f64
```

The distance from the origin.

#### `new`

```rust
pub fn new(x: f64, y: f64) -> Point
```

Construct a new point.

## Trait Implementations

- [`Shape`](Shape.md)
