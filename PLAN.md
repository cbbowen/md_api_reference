# Implementation Plan

This plan implements the spec in [GOALS.md](GOALS.md). It is organized as a dependency
pipeline and delivered in phases that each leave the tool buildable and testable.

## Pipeline overview

```
CLI args ──▶ JSON acquisition ──▶ parse + version check ──▶ reachability
   ──▶ doc model (canonical paths, stubs) ──▶ link map ──▶ render ──▶ write
```

The compiler-facing data (`rustdoc_types::Crate`) is converted into our own **doc model**
early, so rendering never touches raw rustdoc JSON. This keeps rendering testable and
insulates us from rustdoc JSON churn.

## Crate dependencies

| Crate | Purpose |
|-------|---------|
| `clap` (derive) | CLI parsing |
| `rustdoc-types` 0.59 | typed rustdoc JSON |
| `serde_json` | deserialize JSON |
| `rustdoc-json` 0.9 | local JSON generation |
| `ureq` (rustls) | docs.rs download |
| `anyhow` | application error handling ([anyhow-skill]) |

Possible additions if the lightweight approach proves insufficient: `pulldown-cmark`
(+ `pulldown-cmark-to-cmark`) for robust doc-comment markdown rewriting, and `insta`
for snapshot-based golden tests.

## Module layout (`src/`)

```
main.rs            orchestration: parse args → run pipeline
cli.rs             clap definitions + CrateSpec parsing (name / name@version / path)
source.rs          acquire JSON: docs.rs download vs local rustdoc-json, auto-detect
parse.rs           serde_json → rustdoc_types::Crate, format_version check
model/
  mod.rs           DocModel: modules tree, items, canonical vs stub placement
  reachability.rs  graph traversal from roots; pub-use & doc(hidden) handling
  paths.rs         canonical path selection, reexport renaming, collision numbering
render/
  mod.rs           drives file emission from the DocModel
  module.rs        module (lib.md / mod.md) renderer
  type_.rs         struct/enum/union/type-alias file renderer
  trait_.rs        trait file renderer
  signature.rs     format generics, bounds, where-clauses, fn/type signatures
  links.rs         Id → relative md path; intra-doc link & external-item rendering
  doc_text.rs      doc-comment transforms (heading shift, hidden doctest strip)
output.rs          --out empty check, dir creation, file writing
```

## Phases

### Phase 0 — Scaffolding
- Add dependencies; set up `cli.rs` with clap.
- CLI surface: `--crate <SPEC>` (repeatable), `--reexport-crate <SPEC>` (repeatable),
  `--out <DIR>`, `--from-docs-rs` / `--local` overrides, `--manifest-path`, `--target`.
- Parse each `<SPEC>` into a `CrateSpec` (named `name`, `name@version`, or filesystem path).
- `main.rs` wires an empty pipeline that prints the parsed config.
- **Done when:** `--help` is correct and specs parse with unit tests.

### Phase 1 — JSON acquisition + parse
- `source.rs`: auto-detect download vs local; `ureq` GET to
  `https://docs.rs/crate/{crate}/{version}/{target}/json` with a `user-agent`;
  local generation via `rustdoc-json` (`Builder`).
- `parse.rs`: deserialize to `rustdoc_types::Crate`; compare `format_version` against the
  value supported by pinned `rustdoc-types` and error clearly on mismatch.
- **Done when:** we can load a small local fixture crate and a real docs.rs crate into a `Crate`.

### Phase 2 — Reachability + doc model (the core)
- `reachability.rs`: BFS/DFS from each `--crate` root `Module`, descending modules and
  resolving `pub use` (`ItemEnum::Use`) targets, including from private modules and glob
  reexports. Honor `#[doc(hidden)]`. Produce the reachable `Id` set.
- `paths.rs`: for each reachable item compute its public path(s); choose the **canonical**
  one (shortest path, deterministic tie-break); record alternates as **stubs**; apply
  reexport renaming; assign output file paths (with case-collision numbering).
- `model/mod.rs`: assemble a `DocModel` — module tree where each module owns its inline
  items and references to type/trait files; each documented item knows its canonical path.
- **Done when:** unit tests over hand-built fixtures verify reachability, hidden-filtering,
  canonical selection, stub placement, and renamed reexports.

### Phase 3 — Rendering
- `links.rs`: build `Id → relative md path` map from the model; render intra-doc links
  (rustdoc supplies a per-item `links` map of string→Id) — rewrite to relative links when
  the target is documented, otherwise plain code; external items (`std`, deps) as code.
- `signature.rs`: format type definitions and fn signatures (generics, bounds, `where`).
- `doc_text.rs`: shift embedded headings beneath the file heading; strip hidden `# ` doctest
  lines. Start line-based; escalate to `pulldown-cmark` only if needed.
- Renderers, in order: `module.rs` (structure + inline fns/consts/statics/aliases/macros +
  source link + stubs), then `type_.rs` (definition, inherent impls, implemented-traits list
  minus auto/blanket), then `trait_.rs` (supertraits, associated items, in-crate implementors).
- Items grouped by kind in fixed order, alphabetical within group.
- **Done when:** rendering the fixture crate produces correct files for a single crate.

### Phase 4 — Output writing
- `output.rs`: error if `--out` exists and is non-empty; create directories; write files;
  apply `Foo-2.md` disambiguation on case-insensitive collisions.
- **Done when:** end-to-end run writes a correct tree to disk for a single crate.

### Phase 5 — `--reexport-crate`
- Load referenced crates' JSON too; during reachability, follow reexports that cross crate
  boundaries and emit files only for items reexported from a `--crate` (canonical rules apply).
- Handle the separate `Id` spaces / `paths` summaries across crates.
- **Done when:** a fixture facade crate reexporting part of a dependency documents only the
  reexported items.

### Phase 6 — End-to-end verification
- Example fixture crate under `tests/fixtures/example/` exercising: nested modules, all item
  kinds, private-module and renamed reexports, doc(hidden), intra-doc links, generics/bounds.
- Golden output under `tests/golden/`; a test runs the pipeline and diffs (deterministic
  output makes this stable). Consider `insta` for ergonomics.

## Open questions to settle during implementation

1. **Source-link target format.** GOALS says "crate-relative source code link" with a line
   number. Concretely: a relative path to the source file (`src/foo.rs#L12`) rendered from
   each `.md`, or plain text (`src/foo.rs:12`)? rustdoc gives `Item.span` (filename + lines);
   need to confirm the filename base for both local and docs.rs JSON. **Decide in Phase 3.**
2. **Doc-comment rewriting robustness.** Whether the line-based transform suffices or we need
   `pulldown-cmark`. **Revisit in Phase 3.**
3. **HTTP client / TLS.** `ureq` + rustls assumed; confirm it builds cleanly on Windows.
```
