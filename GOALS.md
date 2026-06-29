# Goals

Generate structured markdown documentation for the complete public API of a Rust crate.

The crate(s) to generate documentation for are specified via one or more `--crate` command-line arguments. These are the crates for which the tool generates markdown documentation files. Usually this will be a single crate. However, in some cases it is useful to document multiple crates together, especially for facade crates. The tool runs `rustdoc` on these, generating JSON describing every item, and produces markdown documentation for all publicly accessible items.

This markdown documentation is primarily intended as a reference API to be consumed by LLMs (e.g. copied directly into `references/` for an AI skill). Implementation details should not be documented but can be located as needed by following per-file source code links.

## Documentation Structure

One file per:
* Module (`mod`)
* Type (`struct`, `enum`, `union`, etc.)
* Trait (`trait`)

This structure keeps files relatively shallow.

Modules are represented by directories with a `mod.md` file. For example, given the following module structure in a crate named `example`:

```rust
pub struct Foo;
pub mod top {
    pub struct Bar;
    pub mod inner {
        pub struct Baz;
    }
}
```

This tool will produce the following file structure in the directory specified by the `--out` command-line argument:

```text
└── example/
    ├── lib.md
    ├── Foo.md
    └── top/
        ├── mod.md
        ├── Bar.md
        └── inner/
            ├── mod.md
            └── Baz.md
```

Each module is represented by a directory; that module's own content lives in a file inside the directory. The crate root uses `lib.md` and all other modules use `mod.md`, mirroring Rust's `lib.rs` / `mod.rs` convention. When multiple `--crate` arguments are given, each crate produces its own top-level directory under `--out`.

Every item's own documentation comment (`///` / `//!`) is rendered as markdown in its file (or, for inline items, in its entry within the module file). This includes doc comments on fields, enum variants, associated items, and methods. Doc-comment content is reproduced as faithfully as possible with two transformations:
* Embedded headings (e.g. `# Examples`) are shifted down so they nest beneath the file's own heading structure.
* Hidden doctest lines (those beginning with `# ` inside a code block) are stripped.

All markdown files include a single crate-relative source code link to the location where the item is defined. For modules using the `mod name;` form this is a file link without a line number; for the `mod name { ... }` form and for all other items it includes a line number.

### Ordering

Within a file, items are grouped by kind in a fixed order (e.g. submodules, types, traits, functions, constants, macros), and sorted alphabetically by name within each group. This keeps output deterministic for golden-file testing.

### Module markdown files include:
* Crate-relative source file link (with line number only for the `mod name { ... }` form).
* Submodules (with a reference to the documentation file for that module)
* Types (with a reference to the documentation file for that type)
* Traits (with a reference to the documentation file for that trait)
* Signatures of free-standing functions (i.e. those not in an `impl` block)
* Constants and statics
* Type aliases (`pub type`)
* Macros (`macro_rules!` and procedural macros)

Free functions, constants, statics, type aliases, and macros do not get their own files; they are documented inline within their parent module file.

### Type markdown files include:
* Crate-relative source file and line number link.
* The public parts of the definition (including generics, bounds, and `where` clauses; for enums, the variants and their fields).
* Inherent `impl` blocks, including associated functions, constants, and types, with full signatures.
* A list of the traits the type implements (each linked to its documentation when that trait is itself being documented). Auto traits (e.g. `Send`, `Sync`) and blanket impls are omitted to reduce noise. Trait-impl method bodies/signatures are not repeated here — they live in the trait's file.

### Trait markdown files include:
* Crate-relative source file and line number link.
* Supertraits and the associated functions, constants, and types (with full signatures and default-method presence).
* A list of in-crate implementors (each linked to its documentation).

Accurate markdown cross-references to other items that are being documented are generated throughout all documentation text, including resolved intra-doc links (e.g. `[Foo]`). References to items not being documented (e.g. a `std` type like `Vec`) are rendered as plain text or code, not links.

## Filters

Documentation is only generated for publicly accessible items. Strictly internal items are not included. However, "publicly accessible" includes items that are accessible through the module tree *or through public reexports*. This is determined via a graph reachability search. For example, in this crate:

```rust
mod private {
    pub struct Foo;
    pub struct Bar;
}
pub use private::Foo;
```

Documentation is generated for `Foo` but not for `Bar`.

`#[doc(hidden)]` on an item indicates that it should be ignored regardless of whether it is publicly accessible.

### Items reachable via multiple paths

An item may be reachable via more than one public path (e.g. defined in a private module and reexported, reexported into several modules, or reexported under a new name). Each such item is given a single **canonical file** at one chosen location (the shortest public path, breaking ties deterministically). Every other public path to the item produces a short **stub entry** in the relevant module file that links to the canonical file rather than duplicating content. When an item is reexported under a different name, the canonical file uses the public (reexported) name.

## Public reexports

### From private modules

These are documented *as if they were public items*. From the perspective of users of the crate, such an item is indistinguishable from a public item defined where it is reexported, so it carries **no** origin annotation.

### From other crates

In addition to `--crate`, zero or more `--reexport-crate` arguments may be specified. The tool also runs `rustdoc` on these. However, markdown files are only produced for items that are publicly reexported from one of the crates specified by `--crate` (following the same canonical-location rules above). This is intended for crates that reexport all or part of one of their dependencies.

Unlike private-module reexports, an item reexported from another crate **is** distinguishable to users: they may also depend on that dependency directly and reach the same item by its original path. To make this visible, each such item is annotated with a `_Reexported from_ `dep::module::Item`.` line (the path in the originating dependency). This applies to the reexported item and, for a reexported module, to each item it brings in.

## Rustdoc JSON

### Source selection

The JSON for each crate is obtained one of two ways, selected automatically with explicit overrides available:
* **Download from docs.rs.** Used when a crate is named (optionally `name@version`). The URL structure is `https://docs.rs/crate/{crate}/{version}/{target}/json`, defaulting `version` to `latest` and `target` to `x86_64-unknown-linux-gnu`. A `user-agent` header *must* be provided.
* **Generate locally** via the `rustdoc-json` crate. Used when a path-like value or `--manifest-path` is supplied. The source may be a local crate path or Cargo's cache.

Explicit flags (`--from-docs-rs` / `--local`) override the auto-detection.

### Parsing

Parsing is accomplished using:
* `rustdoc-types`
* `serde_json`

The tool checks the `format_version` field of incoming JSON against the version supported by the pinned `rustdoc-types` and fails with a clear error on mismatch (this is the most likely failure mode for JSON downloaded from docs.rs, which may have been produced by a different toolchain).

`format_version` is **not stable** and is tied to the exact nightly that produced the JSON, so a single pinned `rustdoc-types` cannot read every crate:
* **Local generation** must use a nightly toolchain whose `format_version` matches the pinned `rustdoc-types`. Currently `rustdoc-types` 0.59 ⇒ `format_version` 59, which is emitted by nightly `1.98.0` (2026-06-28) or newer. Keep the toolchain and the pin in lockstep when bumping either.
* **docs.rs downloads** only parse when that crate's most recent docs.rs build used a compatible nightly. Recently-rebuilt crates tend to match the latest `format_version`; long-stable crates may lag (e.g. observed `serde` latest at 57 while `anyhow` was at 59). A mismatch here is expected and reported, not a bug.

## Output directory

The `--out` directory is created if it does not exist. If it already exists and is non-empty, the tool emits an error rather than writing into or overwriting it.

On case-insensitive filesystems (Windows, default macOS), two output paths could in principle collide when item names differ only in case, or when a type name matches a sibling module directory. The naming conventions chosen here make this extremely rare. If it does occur, a disambiguation number is appended to the colliding file name (e.g. `Foo-2.md`).

## Testing / Verification

Unit tests cover reachability analysis, item formatting, and cross-referencing.

End-to-end verification is accomplished via an example crate and golden documentation. Deterministic output (see Ordering) makes golden comparisons stable.
