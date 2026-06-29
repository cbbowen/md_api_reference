//! A facade crate that reexports part of `dep`.

/// A type that belongs to the facade itself.
pub struct Local;

// Reexport a single type from the dependency.
pub use dep::Widget;

// Reexport a whole module from the dependency.
pub use dep::sub;

// Reexport a free function from the dependency.
pub use dep::helper;
