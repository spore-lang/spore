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
fn main() -> Int {
    let greeting = "Hello, Spore!";
    42
}
```

### Structs and Pattern Matching

```spore
struct Point { x: Int, y: Int }

type Shape {
    Circle(Int),
    Rect(Int, Int),
}

fn area(s: Shape) -> Int {
    match s {
        Circle(r) => r * r * 3,
        Rect(w, h) => w * h,
    }
}
```

### Lambdas, Pipes, and Higher-Order Functions

```spore
fn apply(f: (Int) -> Int, x: Int) -> Int { f(x) }

fn main() -> Int {
    let double = |x: Int| x * 2;
    apply(double, 21)
}
```

### Capabilities, Costs, and Error Sets

These annotations are part of the function signature — the compiler verifies
that callers supply the required capabilities and that costs stay within budget.

```spore
fn fetch(url: String) -> String ! [NetError, Timeout]
    cost ≤ 1000
    uses [NetRead]
{
    ?todo
}
```

### Parallel Fetch (Design Intent — not yet runnable)

```spore
fn fetch_all(urls: List[String]) -> List[String] ! [NetError, Timeout]
    cost ≤ urls.len * per_fetch_cost
    uses [NetRead, Spawn]
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
    fn show(self: T) -> String
}

impl Display for Point {
    fn show(self: Point) -> String { "point" }
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

**Compiler infrastructure implemented.** Parser is feature-complete for the current syntax direction. Type checker covers unification, pattern exhaustiveness, trait conformance, error set checking, and cost analysis. Interpreter is a PoC tree-walking evaluator with enum constructors, 30+ builtin functions (list/string/math/IO), method-style dispatch, and try-operator support.

See [docs/DESIGN.md](docs/DESIGN.md) for the implementation-oriented overview in this repo.
Authoritative language and system specifications now live in
[`spore-evolution`](https://github.com/spore-lang/spore-evolution).

## Documentation

### Design and specs
| Document | Description |
|----------|-------------|
| [docs/DESIGN.md](docs/DESIGN.md) | Local implementation-oriented design overview |
| [spore-evolution/VISION.md](https://github.com/spore-lang/spore-evolution/blob/main/VISION.md) | Design philosophy and principles |
| [spore-evolution/ROADMAP.md](https://github.com/spore-lang/spore-evolution/blob/main/ROADMAP.md) | Long-term goals by system area |
| [spore-evolution/seps/](https://github.com/spore-lang/spore-evolution/tree/main/seps) | Authoritative SEPs for syntax, type system, holes, compiler architecture, concurrency, packages, and standard library |

### Design Overview
See [docs/DESIGN.md](docs/DESIGN.md) for the master design document with all confirmed decisions.

## License

MIT OR Apache-2.0
