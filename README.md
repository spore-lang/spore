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
- Stable generic bounds use repeated single-bound clauses: `where T: Trait`.
- Error sets are checked contracts: `throw expr` must match the current `! [...]`, calling a throwing function requires compatible caller errors, and `?` is propagation sugar.
- Primitive syntax is `I32`/`I64`/`U32`/`U64`/`F32`/`F64`/`Bool`/`Char`/`Str`/`()`. Hole syntax stays at the richer docs target in the syntax spec.
- Active concurrency docs target `parallel_scope { ... }`, `spawn { ... }`, postfix `task.await`, `Channel.new[...]`, and `select { ... timeout(...) => ... }`.

## Quick Start

```bash
cargo build                             # build the compiler
cargo run --bin spore -- run demo.sp    # run the demo program (outputs 204)
cargo run --bin spore -- check demo.sp  # type-check only
cargo test --all                        # run all tests
```

## Examples

### Hello World

```spore
fn main() -> I32 {
    let greeting = "Hello, Spore!";
    42
}
```

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

fn main() -> I32 {
    let double = |x: I32| x * 2;
    apply(double, 21)
}
```

### Capabilities, Costs, and Error Sets

These annotations are part of the function signature — the compiler verifies
capabilities, cost budgets, and checked error contracts at call boundaries.
`throw expr` must be covered by the current function's `! [...]`, calling a
`! [...]` function requires a compatible caller signature, and `?` is sugar for
that propagation rule.

The parser accepts `where`, `uses`, `cost`, and `spec` clauses in any order.
Documentation examples use the canonical order: `where`, `uses`, `cost`, `spec`,
and stable `where` syntax is the single-bound form `where T: Trait` (repeat
clauses as needed). Active cost syntax is the fixed-order vector
`cost [compute, alloc, io, parallel]`; each slot currently uses the minimal
subset only: integer constants, parameter variables, or linear `O(n)` terms.
Old scalar `cost <= expr`, `log`/`max`/`min`, and richer algebraic terms are
deferred.

```spore
fn fetch(url: Str) -> Str ! [NetError, Timeout]
    uses [NetRead]
    cost [1, 0, 1, 0]
{
    ?todo
}
```

### Parallel Fetch (Design Intent — not yet runnable)

```spore
fn fetch_all(urls: List[Str], n: I32) -> List[Str] ! [NetError, Timeout]
    uses [NetRead, Spawn]
    cost [O(n), O(n), n, n]
{
    parallel_scope {
        urls |> map(|url| spawn { fetch(url) })
             |> map(|task| task.await?)
    }
}
```

> This illustrates the target syntax for structured concurrency with capability
> propagation. The parser accepts it but the interpreter cannot execute it yet.
> Until richer cost-slot terms land, examples use explicit parameters such as
> `n` instead of projections like `urls.len`.
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
sporec (stateless compiler — pure function)
├── spore-parser     Source text → AST
├── spore-typeck     Type checking, capability & cost analysis
│   ├── hir          HIR with pipe desugaring
│   ├── capability   Capability algebra (∪/∩/hierarchy)
│   ├── cost         4D cost vectors + cost checker
│   ├── hole         Hole dependency graph + topological ordering
│   ├── sig_hash     BLAKE3 256-bit signature hashing
│   ├── incremental  Incremental compilation DB
│   ├── module       Module registry + import resolution
│   ├── concurrency  Structured concurrency analysis
│   └── platform     Platform system (cli/web/embedded)
└── spore-codegen    Tree-walk interpreter (PoC) / Cranelift (planned)

spore (stateful codebase manager — handles IO)
├── File watching, incremental compilation
├── Package management (content-addressed)
├── Platform management
└── LSP server (spore-lsp)
```

## Project Status

**Compiler infrastructure implemented.** Parser is feature-complete for the syntax spec. Type checker covers unification, pattern exhaustiveness, trait conformance, error set checking, and cost analysis. Interpreter is a PoC tree-walking evaluator with enum constructors, 30+ builtin functions (list/string/math/IO), method-style dispatch, and try-operator support.

See [docs/](docs/) for comprehensive specifications.

## Packaging

The `spore` CLI is packaged from `crates/spore-cli` via `maturin` so it can be built and published as a PyPI binary package.

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

In this repository, the local Spore hooks intentionally focus on the canonical demo surface (`demo.sp` today). The reusable published hooks below are for arbitrary `.sp` files in downstream repos.

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

### Specifications
| Document | Description |
|----------|-------------|
| [Syntax Spec](docs/specs/syntax-spec-v0.1.md) | Complete syntax reference |
| [Signature Details](docs/specs/syntax-spec-v0.1.md#appendix-b-signature-details) | Function signature design |
| [Type System](docs/specs/type-system-v0.1.md) | Type system specification |
| [Module System](docs/specs/module-system-v0.1.md) | File-derived modules and dual hash addressing |
| [Effect Algebra](docs/specs/effect-algebra-v0.1.md) | Capability set algebra and composition |
| [Cost Analysis](docs/specs/cost-analysis-v0.1.md) | Cost model and static analysis |
| [Compiler Output](docs/specs/compiler-output-v0.1.md) | Diagnostic format (text / verbose / JSON) |
| [Hole Report v0.3](docs/specs/hole-report-v0.3.md) | Active hole protocol and report format |
| [Hole Dependency Graph](docs/specs/hole-dependency-graph-v0.1.md) | Hole DAG and parallel fill ordering |
| [Concurrency](docs/specs/concurrency-model-v0.1.md) | Structured concurrency model |
| [Package Management](docs/specs/package-management-v0.1.md) | Content-addressed packages |
| [Platform System](docs/specs/platform-system-v0.1.md) | IO through effect handlers |
| [Incremental Compilation](docs/specs/incremental-compilation-v0.1.md) | Watch mode and incremental builds |

### Historical Reference
| Document | Description |
|----------|-------------|
| [Hole System v0.2](docs/archive/hole-system-v0.2.md) | Replaced by the active hole docs; kept as archive only |

### Design Overview
See [docs/DESIGN.md](docs/DESIGN.md) for the master design document with all confirmed decisions.

## License

MIT OR Apache-2.0
