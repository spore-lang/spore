# Spore (孢子)

**A compiled, general-purpose programming language where the function signature is the single source of truth for intent — built for human–Agent collaborative development.**

Spore treats every function signature as a complete specification: input/output types, error sets, required effects, computational cost budgets, and `spec` assertions live in one place and are verified by the compiler. Programs with `?holes` compile successfully, making "partially-implemented" a normal working state and giving humans and Agents a shared, structured collaboration surface (`HoleReport`).

## Key Features

- **Signatures as the single source of truth** — `where` / `uses` / `cost` / `spec` clauses carry constraints, required effects, cost budgets, and behavior assertions in one place; no scattered docs or stubs to keep in sync.
- **Hole system as a collaboration protocol** — `?name` partial functions are first-class; the compiler emits the same structured context for humans and Agents to fill in dependency order.
- **Effect system** — IO effects are declared on signatures and verified at compile time; all IO flows through Platform-provided effect handlers, keeping application code pure.
- **Cost model** — 4-dimension cost vectors (`compute`, `alloc`, `io`, `parallel`) checked against compile-time budgets.
- **Content-addressed modules** — dual signature/implementation hashes drive incremental compilation and caching; no semver, no diamond dependencies. `spore lock` ships the first local content-hash lock/store slice (registry distribution is roadmap).
- **Structured concurrency** — `parallel_scope` / `spawn` / `Channel` / `select` with cancellation propagation; no async coloring.
- **Expression-based** — everything is an expression; recursion and higher-order functions replace loops.

## Install

Spore is in alpha. Two install paths are supported:

### `cargo` — for source / latest main

Requires Rust 1.95+.

```bash
# install the latest commit on main
cargo install --git https://github.com/spore-lang/spore spore --locked

# or, from a local checkout
cargo install --path crates/spore --locked
```

### `uv` — recommended for end users

The alpha CLI is published to PyPI as `spore-lang`. Requires Python 3.13+.

```bash
uv tool install spore-lang
spore --version
```

## Canonical Surface Syntax

- Modules come only from file paths; there is no `module ...` header.
- Effect checks live on function signatures and package/Platform boundaries only; source files have no module-level `uses` carrier.
- Stable generic bounds use a single comma-separated clause: `where T: Trait, U: Trait`.
- Effect operations use explicit `effect` declarations plus `perform Effect.op(...)`; reusable unions use `effect Name = A | B`.
- Error sets are checked contracts: `throw expr` must match the current `! E1 | E2`, calling a throwing function requires compatible caller errors, and `?` is propagation sugar.
- Current implementation primitives are fixed-width only: `I8`/`I16`/`I32`/`I64`, `U8`/`U16`/`U32`/`U64`, `F32`/`F64`, `Bool`, `Str`, `Never`, and `()`. `Int` and `Float` are not built-in aliases.
- The live structured-concurrency surface includes `parallel_scope { ... }`, `spawn { ... }`, postfix `task.await`, `Channel.new[...]`, and `select { ... timeout(...) => ... }`.

## Quick Start

```bash
cargo build                                      # build the compiler
cargo run --bin spore -- new hello-app          # create a new application project
cd hello-app && ../target/debug/spore run src/main.sp  # run the application from this checkout
cargo test --all                                 # run all compiler tests
```

For single-file exploration:

```bash
cargo run --bin spore -- run examples/demo.sp   # run standalone file (no Platform)
cargo run --bin spore -- check examples/demo.sp # type-check standalone file
cargo run --bin spore -- test examples/demo.sp  # validate spec examples in file
```

For manifest-backed projects, the first local content-addressed lock/store slice
is available via:

```bash
cargo run --bin spore -- lock path/to/project         # write .spore-lock + .spore-store
cargo run --bin spore -- lock --check path/to/project # CI check for lock freshness
```

This MVP snapshots the root package plus local path dependencies by deterministic
content hash. The project resolver can also use `.spore-lock` entries to resolve
manifest dependencies that omit `path = ...`, loading their stored source roots
from `.spore-store/packages/<content-hash>`. Registry backends,
signature-level dependency acceptance, and full package update workflows remain
roadmap items.

If `spore` is installed on your `PATH`, you can replace the explicit Cargo or
`target/debug/spore` invocations above with bare `spore ...`.

## Examples

### Hello World (Application Project)

The canonical way to write Spore programs is as a project with a Platform contract:

```bash
spore new hello-app
```

This generates `src/main.sp` with a Platform-aware entry point:

```spore
import basic_cli.stdout

fn main() -> () uses [Console] {
    println("Hello from hello-app!")
    return
}
```

Applications declare `fn main() -> ()` and require effects that are handled by the Platform.
The `basic-cli` Platform handles effect operations like `Console` for terminal IO.

Platform packages can be scaffolded too:

```bash
spore new --type platform my-platform
```

That scaffold now includes `src/platform_contract.sp` plus matching `[platform]`
metadata in `spore.toml`, so application projects can point at it via a path
dependency while `src/host.sp` remains a local smoke entry.

### Standalone File Mode

For quick experiments, you can run single `.sp` files without a project:

```spore
fn demo() -> I32 {
    let x = 42;
    x * 2
}

fn main() -> I32 {
    demo()
}
```

Standalone mode does not participate in a package-backed Platform contract and does
not have access to platform builtins such as `println`. Console I/O requires a
project with an explicit Platform effect boundary (e.g. `basic-cli`).
See [`examples/demo.sp`](examples/demo.sp) for a standalone example file.

### Structs and Pattern Matching

```spore
struct Point { x: I32, y: I32 }

type Shape {
    Circle(I32),
    Rect(I32, I32),
}

fn area(s: Shape) -> I32 {
    match s {
        Circle(r) => r * r * 3,
        Rect(w, h) => w * h,
    }
}
```

### Lambdas, Pipes, and Higher-Order Functions

```spore
fn apply(f: (I32) -> I32, x: I32) -> I32 { f(x) }

fn compute() -> I32 {
    let double = |x: I32| x * 2;
    apply(double, 21)
}
```

### Required Effects, Costs, and Error Sets

These annotations are part of the function signature — the compiler verifies
required effects, cost budgets, checked error contracts, and explicit effect
surfaces at call boundaries. `throw expr` must be covered by the current
function's `! E1 | E2`, calling a throwing function requires a compatible
caller signature, and `?` is sugar for
that propagation rule.

The parser accepts `where`, `uses`, `cost`, and `spec` clauses in any order.
Documentation examples use the canonical order: `where`, `uses`, `cost`, `spec`,
and stable `where` syntax is a single comma-separated clause such as
`where T: Trait, U: Trait`. Active cost syntax is the fixed-order vector
`cost [compute, alloc, io, parallel]`; each slot currently uses the minimal
subset only: integer constants, parameter variables, or linear `O(n)` terms.
Old scalar `cost <= expr`, `log`/`max`/`min`, and richer algebraic terms are
deferred. Functions marked `@unbounded` are still contagious and skip body
budget verification, but they must declare an expected vector with the same
`cost [compute, alloc, io, parallel]` syntax so callers and docs preserve an
explicit cost intent.

```spore
effect NetConnect {
    fn fetch(url: Str) -> Str ! NetError | Timeout
}

fn fetch(url: Str) -> Str ! NetError | Timeout
    uses [NetConnect]
    cost [1, 0, 1, 0]
{
    perform NetConnect.fetch(url)
}
```

### Parallel Fetch

```spore
fn fetch_all(urls: List[Str], n: I32) -> List[Str] ! NetError | Timeout
    uses [NetConnect, Spawn]
    cost [O(n), O(n), n, n]
{
    parallel_scope {
        urls |> map(|url| spawn { fetch(url) })
             |> map(|task| task.await?)
    }
}
```

> This is part of the live structured-concurrency surface: the parser,
> typechecker, and interpreter all cover the current `parallel_scope` / `spawn`
> / `await` / `select` subset. Until richer cost-slot terms land, examples use
> explicit parameters such as `n` instead of projections like `urls.len`.
> The same active-docs target also uses `Channel.new[...]` and
> `select { msg from rx => ..., timeout(5.seconds) => ... }`.

### Traits and Implementations

```spore
trait Display[T] {
    fn show(self: T) -> Str
}

impl Display for Point {
    fn show(self: Point) -> Str { "point" }
}
```

### Compositional stdlib modules

The first compositional stdlib slice now ships under the target engineering-facing
module names:

```spore
import spore.merge
import spore.laws

fn canonical_members(xs: List[I32]) -> List[I32] {
    canonical_members_i32(xs)
}
```

Today this is intentionally small and truthful to the live implementation:

- `spore.combine` provides higher-order combine helpers plus a `Combine[T]` trait
- `spore.merge` provides list-backed unique merge helpers plus a `Merge[T]` trait
- `spore.order` layers small helpers over the current prelude `Ordering`
- `spore.laws` hosts executable law-oriented helpers such as
  `canonical_members_i32` and `sum3_left_assoc_i32`, expressed with ordinary
  `spec { example ... property ... }` clauses

`spore.laws` does **not** add trusted optimizer rules or new `law` syntax yet.
It gives you reusable helpers plus patterns you can copy into your own APIs
today. For example, you can encode an associative check for a local combine step
with existing surface syntax:

```spore
import spore.combine

fn sum3(a: I32, b: I32, c: I32) -> I32
spec {
    property "associative": |a: I32, b: I32, c: I32|
        combine_pair(a, combine_pair(b, c, |x: I32, y: I32| x + y), |x: I32, y: I32| x + y)
}
{
    combine_pair(combine_pair(a, b, |x: I32, y: I32| x + y), c, |x: I32, y: I32| x + y)
}
```

Or you can reuse the shipped helpers directly:

```spore
import spore.laws

fn unique_ids(xs: List[I32]) -> List[I32] {
    canonical_members_i32(xs)
}

fn total(a: I32, b: I32, c: I32) -> I32 {
    sum3_left_assoc_i32(a, b, c)
}
```

That is the current model for law tooling: executable helpers, reusable
stdlib examples, and documented `spec` patterns — not a new core proof system.

Older shipped stdlib helpers still live on the prelude or legacy root-module
surface (`math`, `set`, `dict`, ...); this new `spore.*` naming is the current
targeted first slice, not a full rename of every shipped module yet.

## Architecture

```
sporec (stateless compiler CLI / product)
└── sporec-driver    Host-side compiler driver crate
    ├── sporec-parser     Source text -> AST
    ├── sporec-typeck     Type checking, effect & cost analysis
    │   ├── hir          HIR with pipe desugaring
    │   ├── effect sets  Effect-set algebra (∪/∩/hierarchy)
    │   ├── cost         4D cost vectors + cost checker
    │   ├── hole         Hole dependency graph + topological ordering
    │   ├── sig_hash     BLAKE3 256-bit signature hashing
    │   ├── incremental  Incremental compilation DB
    │   ├── module       Module registry + import resolution
    │   ├── concurrency  Structured concurrency analysis
    │   └── platform     Platform system (cli/web/embedded)
    └── sporec-codegen   Tree-walk interpreter (PoC) / experimental native object backend for a supported scalar/object subset

spore (stateful codebase manager — handles IO / project workflow)
├── File watching, incremental compilation
├── Project scaffolding and package metadata (`spore lock` local content-hash store; registry/update workflow is roadmap)
├── Platform management
└── LSP server (spore-lsp)
```

## Project Status

**Compiler infrastructure implemented.** Parser is feature-complete for the syntax spec. Type checker covers unification, pattern exhaustiveness, trait conformance, error set checking, cost analysis, and the current structured-concurrency subset. Interpreter is a PoC tree-walking evaluator with enum constructors, 30+ builtin functions (list/string/math/IO), method-style dispatch, try-operator support, and the current structured-concurrency runtime.

Native build support is experimental: the current backend emits object files for
the supported scalar/object subset and rejects unsupported language features
explicitly. General native compilation remains future work.

See [SPARK.md](SPARK.md) for the project vision and design direction. Topic-level normative proposals live in the sibling [`spore-evolution`](https://github.com/spore-lang/spore-evolution/tree/main/seps) repo under `seps/`.

## Packaging

The `spore` CLI is packaged from `crates/spore` via `maturin` so it can be
built and published as a PyPI binary package.

```bash
just package-cli        # build a wheel into dist/
just package-cli-sdist  # build a source distribution into dist/
```

GitHub Actions builds wheel artifacts for Linux x86_64, macOS x86_64, macOS
arm64, and Windows x86_64 on every push and pull request. Pushing a `v*` tag is
wired to run `.github/workflows/cd-publish.yml`, which builds the same wheel
matrix plus an sdist, smoke-tests the packaged artifacts, uploads those
artifacts to GitHub Releases, and publishes the collected distributions to PyPI
from the `pypi` environment via trusted publishing. The next public tag-driven
release is still blocked until that GitHub `pypi` environment and the matching
PyPI trusted publisher configuration are in place.

The current MSRV is Rust 1.95, matching `Cargo.toml`, `rust-toolchain.toml`,
CI, and `just msrv`.

## Development

### Local hooks

```bash
just pre-commit-install  # install pre-commit + commit-msg hooks via prek
just pre-commit          # run the configured hooks on all files
```

In this repository, the local Spore hooks intentionally focus on the canonical example surface under `examples/`. The reusable published hooks below are for arbitrary `.sp` files in downstream repos.

### Reusable hooks

With the root `pyproject.toml` in place, the repository can also expose reusable pre-commit hooks for `.sp` files:

```yaml
repos:
  - repo: https://github.com/spore-lang/spore-pre-commit
    rev: <tag-or-sha>
    hooks:
      - id: spore-format
      - id: spore-check
```

The dedicated thin mirror lives at
[`spore-lang/spore-pre-commit`](https://github.com/spore-lang/spore-pre-commit).
Hook installation still builds Spore from source, so consumers need a working
Rust 1.95+ toolchain.

## Documentation

### Canonical design docs

| Document                                           | Description                                           |
| -------------------------------------------------- | ----------------------------------------------------- |
| [SPARK.md](SPARK.md)                               | Project vision, design direction, and core principles |
| [docs/specs/README.md](docs/specs/README.md)       | Redirect for the retired per-topic spec drafts        |
| [docs/research/README.md](docs/research/README.md) | Redirect for the retired research drafts              |

### SEP mapping

Detailed topic proposals now live in [`spore-evolution/seps/`](https://github.com/spore-lang/spore-evolution/tree/main/seps):

- [`SEP-0001-core-syntax.md`](https://github.com/spore-lang/spore-evolution/blob/main/seps/SEP-0001-core-syntax.md)
- [`SEP-0002-type-system.md`](https://github.com/spore-lang/spore-evolution/blob/main/seps/SEP-0002-type-system.md)
- [`SEP-0003-effect-system.md`](https://github.com/spore-lang/spore-evolution/blob/main/seps/SEP-0003-effect-system.md)
- [`SEP-0004-cost-analysis.md`](https://github.com/spore-lang/spore-evolution/blob/main/seps/SEP-0004-cost-analysis.md)
- [`SEP-0005-hole-system.md`](https://github.com/spore-lang/spore-evolution/blob/main/seps/SEP-0005-hole-system.md)
- [`SEP-0006-compiler-architecture.md`](https://github.com/spore-lang/spore-evolution/blob/main/seps/SEP-0006-compiler-architecture.md)
- [`SEP-0007-concurrency-model.md`](https://github.com/spore-lang/spore-evolution/blob/main/seps/SEP-0007-concurrency-model.md)
- [`SEP-0008-module-package-system.md`](https://github.com/spore-lang/spore-evolution/blob/main/seps/SEP-0008-module-package-system.md)
- [`SEP-0009-standard-library.md`](https://github.com/spore-lang/spore-evolution/blob/main/seps/SEP-0009-standard-library.md)

## License

MIT
