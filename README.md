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
that callers supply the required capabilities and that costs stay within budget.

The parser accepts `where`, `uses`, `cost`, and `spec` clauses in any order.
Documentation examples use the canonical order: `where`, `uses`, `cost`, `spec`.

```spore
fn fetch(url: Str) -> Str ! [NetError, Timeout]
    uses [NetRead]
    cost ≤ 1000
{
    ?todo
}
```

### Parallel Fetch (Design Intent — not yet runnable)

```spore
fn fetch_all(urls: List[Str]) -> List[Str] ! [NetError, Timeout]
    uses [NetRead, Spawn]
    cost ≤ urls.len * per_fetch_cost
{
    parallel_scope {
        urls |> map(|url| spawn { fetch(url) })
             |> map(|task| task.await?)
    }
}
```

> This illustrates the target syntax for structured concurrency with capability
> propagation. The parser accepts it but the interpreter cannot execute it yet.

### Capabilities and Implementations

```spore
capability Display[T] {
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

## Documentation

### Specifications
| Document | Description |
|----------|-------------|
| [Syntax Spec](docs/specs/syntax-spec-v0.1.md) | Complete syntax reference |
| [Signature Details](docs/specs/syntax-spec-v0.1.md#appendix-b-signature-details) | Function signature design |
| [Type System](docs/specs/type-system-v0.1.md) | Type system specification |
| [Module System](docs/specs/module-system-v0.1.md) | Module & dual hash system |
| [Hole System](docs/specs/hole-system-v0.2.md) | Hole system for Agent collaboration |
| [Cost Model](docs/specs/cost-model-v0.1.md) | 4-dimension cost analysis |
| [Compiler Output](docs/specs/compiler-output-v0.1.md) | Diagnostic format (3 modes) |
| [Concurrency](docs/specs/concurrency-model-v0.1.md) | Structured concurrency model |
| [Package Management](docs/specs/package-management-v0.1.md) | Content-addressed packages |
| [Platform System](docs/specs/platform-system-v0.1.md) | IO through effect handlers |
| [Incremental Compilation](docs/specs/incremental-compilation-v0.1.md) | Watch mode & incremental builds |
| [Effect Algebra](docs/specs/effect-algebra-v0.1.md) | Capability set algebra & composition |
| [Recursion Analysis](docs/specs/recursion-analysis-v0.1.md) | Three-tier recursive cost analysis |
| [Cost Decidability](docs/specs/cost-decidability-v0.1.md) | CostExpr grammar & decidability proof |
| [Hole Report v0.3](docs/specs/hole-report-v0.3.md) | Extended HoleReport & Agent protocol |
| [Hole Dependency Graph](docs/specs/hole-dependency-graph-v0.1.md) | Hole DAG & parallel fill algorithm |

### Design Overview
See [docs/DESIGN.md](docs/DESIGN.md) for the master design document with all confirmed decisions.

## License

MIT OR Apache-2.0
