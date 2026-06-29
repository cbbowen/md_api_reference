//! Example crate used as a documentation fixture.
//!
//! Mirrors the module layout described in `GOALS.md`.

/// A crate-root type.
pub struct Foo;

pub mod top {
    /// A type nested one level deep.
    pub struct Bar;

    pub mod inner {
        /// A type nested two levels deep.
        pub struct Baz;
    }
}

mod private {
    /// Reachable only via the reexport below.
    pub struct Reexported;

    /// Never reachable through a public path.
    pub struct Hidden;
}

pub use private::Reexported;
