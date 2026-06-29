# Trait `example::Shape`

_Defined at_ `src/lib.rs:54`

A geometric shape.

## Declaration

```rust
pub trait Shape
```

## Associated Types

### `Output`

```rust
type Output
```

The numeric type produced by measurements.

## Associated Constants

### `SIDES`

```rust
const SIDES: u32
```

How many sides the shape has.

## Required and Provided Methods

### `area`

```rust
fn area(&self) -> Self::Output
```

The area of the shape.

## Implementors

- [`Point`](Point.md)
