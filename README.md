# Spore (孢子)

**A general-purpose programming language optimized for Agent-human intent interaction.**

Spore is a compiled language where function signatures are "gravity centers" — complete specifications carrying type, error, effect, cost, and capability information. The compiler serves as a documentation assistant, and the hole system enables Agent-driven collaborative development.

## Key Features

- **Hole System**: `?name` partial functions as first-class collaboration protocol with Agents
- **Capability System**: IO effects gated by capabilities, verified at compile time
- **Cost Model**: 4-dimension cost analysis (compute, alloc, io, parallel) with compile-time budgets
- **Content-Addressed**: Dual hash (sig + impl) for modules — no semver, pure hash addressing
- **Effect Handlers**: All IO through Platform-provided effect handlers, application code stays pure
- **Structured Concurrency**: Task trees with cancellation propagation, channels for communication
- **Expression-Based**: Everything is an expression, no loops (recursion + higher-order functions)

## Canonical Surface Syntax

- Modules come only from file paths; there is no `module ...` header.
- Capability checks live on function signatures and package/Platform boundaries only; source files have no module-level `uses` carrier.
- Stable generic bounds use a single comma-separated clause: `where T: Trait, U: Trait`.
- Effect operations use explicit `effect` declarations plus `perform Effect.op(...)`; reusable unions use `effect Name = A | B`.
- Error sets are checked contracts: `throw expr` must match the current `! E1 | E2`, calling a throwing function requires compatible caller errors, and `?` is propagation sugar.
- Current implementation primitives are `I32`/`I64`/`U32`/`U64`/`F32`/`F64`/`Bool`/`Char`/`Str`/`()`. The locked surface also keeps `Int`/`Float` as `I64`/`F64` aliases; implementation catch-up is tracked separately.
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

Applications declare `fn main() -> ()` and use Platform-provided capabilities.
The `basic-cli` Platform handles effect operations like `Console` for terminal IO.

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

Standalone mode uses `fn main() -> I32` and does not participate in a package-backed Platform contract.
It still runs through legacy built-in CLI behavior today (e.g., `println` works), but the return value is printed as output rather than used as a process exit code.
Real applications should prefer `spore new` / project mode with Platform capabilities.
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

### Capabilities, Costs, and Error Sets

These annotations are part of the function signature — the compiler verifies
capabilities, cost budgets, checked error contracts, and explicit effect
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
deferred.

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

### Capabilities and Implementations

```spore
trait Display[T] {
    fn show(self: T) -> Str
}

impl Display for Point {
    fn show(self: Point) -> Str { "point" }
}
```

## Architecture

```
sporec (stateless compiler CLI / product)
└── sporec-driver    Host-side compiler driver crate
    ├── sporec-parser     Source text -> AST
    ├── sporec-typeck     Type checking, capability & cost analysis
    │   ├── hir          HIR with pipe desugaring
    │   ├── capability   Capability algebra (∪/∩/hierarchy)
    │   ├── cost         4D cost vectors + cost checker
    │   ├── hole         Hole dependency graph + topological ordering
    │   ├── sig_hash     BLAKE3 256-bit signature hashing
    │   ├── incremental  Incremental compilation DB
    │   ├── module       Module registry + import resolution
    │   ├── concurrency  Structured concurrency analysis
    │   └── platform     Platform system (cli/web/embedded)
    └── sporec-codegen   Tree-walk interpreter (PoC) / Cranelift (planned)

spore (stateful codebase manager — handles IO / project workflow)
├── File watching, incremental compilation
├── Package management (content-addressed)
├── Platform management
└── LSP server (spore-lsp)
```

## Project Status

**Compiler infrastructure implemented.** Parser is feature-complete for the syntax spec. Type checker covers unification, pattern exhaustiveness, trait conformance, error set checking, cost analysis, and the current structured-concurrency subset. Interpreter is a PoC tree-walking evaluator with enum constructors, 30+ builtin functions (list/string/math/IO), method-style dispatch, try-operator support, and the current structured-concurrency runtime.

See [docs/DESIGN.md](docs/DESIGN.md) for the canonical in-repo design document. Topic-level normative proposals live in the sibling `spore-evolution/seps/` repo.

## Packaging

The `spore` CLI is packaged from `crates/spore` via `maturin` so it can be built and published as a PyPI binary package.

```bash
just package-cli        # build a wheel into dist/
just package-cli-sdist  # build a source distribution into dist/
```

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
  - repo: https://github.com/spore-lang/spore
    rev: <tag-or-sha>
    hooks:
      - id: spore-format
      - id: spore-check
```

Until a dedicated thin mirror repo exists, installing these hooks from the source repo still builds Spore from source, so consumers need a working Rust toolchain.

## Documentation

### Canonical design docs
| Document | Description |
|----------|-------------|
| [docs/DESIGN.md](docs/DESIGN.md) | Primary in-repo design document, syntax authority, and durable design summary |
| [docs/specs/README.md](docs/specs/README.md) | Redirect for the retired per-topic spec drafts |
| [docs/research/README.md](docs/research/README.md) | Redirect for the retired research drafts |

### SEP mapping
Detailed topic proposals now live in the sibling `spore-evolution/seps/` repo:

- `SEP-0001-core-syntax.md`
- `SEP-0002-type-system.md`
- `SEP-0003-effect-capability-system.md`
- `SEP-0004-cost-analysis.md`
- `SEP-0005-hole-system.md`
- `SEP-0006-compiler-architecture.md`
- `SEP-0007-concurrency-model.md`
- `SEP-0008-module-package-system.md`
- `SEP-0009-standard-library.md`

## License

MIT OR Apache-2.0
