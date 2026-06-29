//! A dependency crate that a facade reexports from.

/// A widget defined in `dep`.
pub struct Widget {
    /// The widget's identifier.
    pub id: u32,
}

impl Widget {
    /// Make a widget.
    pub fn new(id: u32) -> Widget {
        Widget { id }
    }
}

/// A free helper in `dep`.
pub fn helper() -> u32 {
    42
}

/// A type that will NOT be reexported by the facade.
pub struct Unexported;

/// A gizmo that the facade reexports under a different name.
pub struct Gizmo;

pub mod sub {
    /// A type inside `dep::sub`.
    pub struct Inner;
}
