//! Example crate used as a documentation fixture.
//!
//! Mirrors the module layout described in `GOALS.md`, and exercises the
//! renderer across item kinds.

/// A crate-root type.
pub struct Foo;

/// A configuration value.
pub const MAX_DEPTH: u32 = 8;

/// A convenient alias.
pub type Pair = (u32, u32);

/// Greet by name, returning the greeting.
///
/// The greeting has nothing to do with a [`Point`], but this links to one to
/// exercise intra-doc link resolution.
pub fn greet<S: Into<String>>(name: S) -> String {
    format!("Hello, {}!", name.into())
}

/// A point with named fields.
///
/// A `Point` implements [`Shape`]. See also [`greet`].
pub struct Point {
    /// The horizontal coordinate.
    pub x: f64,
    /// The vertical coordinate.
    pub y: f64,
}

impl Point {
    /// Construct a new point.
    pub fn new(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    /// The distance from the origin.
    pub fn magnitude(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

impl Shape for Point {
    type Output = f64;
    const SIDES: u32 = 0;
    fn area(&self) -> f64 {
        0.0
    }
}

/// A geometric shape.
pub trait Shape {
    /// The numeric type produced by measurements.
    type Output;

    /// How many sides the shape has.
    const SIDES: u32;

    /// The area of the shape.
    fn area(&self) -> Self::Output;
}

/// An RGB or named color.
pub enum Color {
    /// A named color.
    Named(String),
    /// A literal RGB triple.
    Rgb { r: u8, g: u8, b: u8 },
    /// The default.
    Transparent,
}

pub mod top {
    /// A type nested one level deep.
    pub struct Bar;

    // Reexport a crate-root type here too, creating a second (longer) path to
    // `Foo`. Its canonical home stays `example::Foo`; this becomes a stub.
    pub use crate::Foo;

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

mod bulk {
    /// First item exposed via a glob reexport.
    pub struct Alpha;

    /// Second item exposed via a glob reexport.
    pub struct Beta;
}

// Blanket/glob reexport: brings every public item of `bulk` into the crate root.
pub use bulk::*;

/// Public but hidden from docs.
#[doc(hidden)]
pub struct HiddenButPublic;
