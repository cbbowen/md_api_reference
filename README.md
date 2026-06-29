# md_api_reference

> [!IMPORTANT]
> This is almost 100% vibe-coded slop. It's likely not fit for any purpose, even viewing. Frankly, the implementation challenges were rote and wholly uninteresting. The only non-trivial question was exactly what to output. I created it in the hope that it could help make future vibe-coded projects modestly less sloppy.

Generate structured **Markdown documentation for the complete public API** of a Rust
crate — one file per module, type, and trait — primarily so it can be consumed by LLMs
(for example, dropped into a `references/` directory for an AI skill).

It works from [rustdoc's JSON output](https://github.com/rust-lang/rust/issues/76578),
either downloaded from [docs.rs](https://docs.rs) or generated locally, and emits a
clean directory tree of cross-linked Markdown.

> Status: early (`0.1`). The full pipeline works end to end and is covered by unit tests
> plus a golden end-to-end test. See [GOALS.md](GOALS.md) for the specification.

## What it produces

Given a crate like:

```rust
pub struct Foo;
pub mod top {
    pub struct Bar;
    pub mod inner {
        pub struct Baz;
    }
}
```

it writes:

```text
out/
└── example/
    ├── lib.md            # the crate root module
    ├── Foo.md
    └── top/
        ├── mod.md        # the `top` module
        ├── Bar.md
        └── inner/
            ├── mod.md
            └── Baz.md
```

- **One file per** module (`mod`), type (`struct`/`enum`/`union`), and trait. Modules are
  directories containing a `mod.md` (`lib.md` at the crate root).
- **Inline in the module file:** free functions, constants, statics, type aliases, and
  macros (with full signatures).
- **Type files** include the public definition, inherent impls (with method signatures and
  docs), and the list of traits implemented (auto traits and blanket impls omitted).
- **Trait files** include the declaration, associated items, and in-crate implementors.
- **Cross-references** between documented items are resolved to relative links; references
  to undocumented items (e.g. `std`) render as plain code. Each item carries a
  crate-relative source location (e.g. `` `src/lib.rs:6` ``).
- Output is **deterministic** (grouped by kind, alphabetical within each group).

## What gets documented

Only **publicly accessible** items, determined by graph reachability from the crate root:

- Items reachable through the public module tree.
- Items reachable through **public reexports**, including from private modules and via
  glob (`pub use private::*`) and renamed (`pub use x::Y as Z`) reexports. A reexport from
  a private module is documented as if it were defined where it is reexported.
- `#[doc(hidden)]` items are excluded (rustdoc strips them from the JSON).

## Installation

Build from source:

```sh
cargo build --release
# binary at target/release/md_api_reference
```

Generating JSON from a **local** crate requires a **nightly** toolchain whose rustdoc JSON
`format_version` matches the pinned `rustdoc-types` (currently `0.59` ⇒ `format_version`
59, emitted by nightly `1.98.0` or newer). See [Caveats](#caveats).

## Usage

```text
md_api_reference --crate <SPEC> [--crate <SPEC> ...] --out <DIR> [OPTIONS]

  --crate <SPEC>           Crate to document: a crate name, `name@version`, or a path
                           to a local crate. Repeatable.
  --reexport-crate <SPEC>  Additional crate whose items are documented only where
                           publicly reexported from a `--crate`. Repeatable.
  --out <DIR>              Output directory. Must be empty or not yet exist.
  --from-docs-rs           Force downloading rustdoc JSON from docs.rs for every crate.
  --local                  Force generating rustdoc JSON locally for every crate.
  --manifest-path <PATH>   Manifest path used when generating JSON locally.
  --target <TRIPLE>        Target triple for docs.rs downloads
                           (default: x86_64-unknown-linux-gnu).
```

A `<SPEC>` is auto-detected: a name (or `name@version`) is downloaded from docs.rs, while
a path (contains a separator, starts with `.`, ends in `.toml`, or exists on disk) is
generated locally. `--from-docs-rs` / `--local` override the detection.

### Examples

Download a published crate from docs.rs and document it:

```sh
md_api_reference --crate anyhow --out ./docs
```

Pin a version:

```sh
md_api_reference --crate serde@1.0.219 --out ./docs
```

Document a local crate:

```sh
md_api_reference --crate ./path/to/mycrate --out ./docs
```

Document a facade crate together with the dependency it reexports. Items reexported from
the dependency are documented under the facade and annotated with their original path
(e.g. *Reexported from `dep::Widget`*):

```sh
md_api_reference --crate ./facade --reexport-crate ./dep --out ./docs
```

## How it works

```text
CLI → acquire JSON → parse + format_version check → reachability
    → doc model (canonical paths, stubs) → render → write
```

1. **Acquire** the rustdoc JSON per crate: download the zstd-compressed blob from docs.rs,
   or generate it locally with [`rustdoc-json`](https://crates.io/crates/rustdoc-json).
2. **Parse** with [`rustdoc-types`](https://crates.io/crates/rustdoc-types), checking the
   `format_version` up front.
3. **Reachability**: walk public modules and reexports to find the documented item set and
   each item's canonical location (shortest public path; alternate paths become stubs).
4. **Render** module, type, and trait files; **write** them under `--out`.

Cross-crate reexports (from `--reexport-crate`) are resolved by inlining the referenced
item's definition from the dependency's JSON into the primary crate before rendering.

## Caveats

- **rustdoc JSON `format_version` is not stable.** It is tied to the exact nightly that
  produced the JSON, so a single pinned `rustdoc-types` cannot read every crate:
  - Local generation needs a nightly that matches the pin (see [Installation](#installation)).
  - docs.rs downloads only parse if that crate's most recent docs.rs build used a
    compatible nightly. A mismatch is reported with a clear error, not a crash.
- The output directory must be empty (or not exist); the tool refuses to overwrite a
  non-empty directory.
- Source references are rendered as plain `path:line` text, not hyperlinks (the source
  files are not part of the output tree).

## Development

```sh
cargo test            # unit tests + the golden end-to-end test
```

The golden test ([tests/golden.rs](tests/golden.rs)) renders committed JSON fixtures
([tests/fixtures/](tests/fixtures/)) and diffs the result against
[tests/golden/](tests/golden/), so it is hermetic and needs no toolchain. After an
intentional change to the renderer:

```sh
BLESS=1 cargo test --test golden    # regenerate the golden files
```

If the *fixture crates* themselves change, regenerate their JSON first — see the header of
[tests/golden.rs](tests/golden.rs) for the exact command.
