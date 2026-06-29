# Enum `example::Color`

_Defined at_ `src/lib.rs:66`

An RGB or named color.

## Definition

```rust
pub enum Color {
    /// A named color.
    Named(String),
    /// A literal RGB triple.
    Rgb {
        r: u8,
        g: u8,
        b: u8,
    },
    /// The default.
    Transparent,
}
```
