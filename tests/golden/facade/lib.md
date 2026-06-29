# Crate `facade`

_Defined at_ `src/lib.rs:1`

A facade crate that reexports part of `dep`.

## Modules

- [`sub`](sub/mod.md)

## Types

- [`Local`](Local.md) — A type that belongs to the facade itself.
- [`RenamedGizmo`](RenamedGizmo.md) — A gizmo that the facade reexports under a different name.
- [`Widget`](Widget.md) — A widget defined in `dep`.

## Functions

### `helper`

_Reexported from_ `dep::helper`.

```rust
pub fn helper() -> u32
```

A free helper in `dep`.
