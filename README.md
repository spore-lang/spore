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

## Architecture

```
sporec (stateless compiler — pure function)
├── spore-parser     Source text → AST
├── spore-typeck     Type checking, capability & cost analysis
└── spore-codegen    Typed AST → native code (Cranelift)

spore (stateful codebase manager — handles IO)
├── File watching, incremental compilation
├── Package management (content-addressed)
├── Platform management
└── LSP server (spore-lsp)
```

## Project Status

**Design phase complete.** See [docs/](docs/) for comprehensive specifications.

## Documentation

### Specifications
| Document | Description |
|----------|-------------|
| [Syntax Spec](docs/specs/syntax-spec-v0.1.md) | Complete syntax reference |
| [Signature Syntax](docs/specs/signature-syntax-v0.2.md) | Function signature design |
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

## Quick Example

```spore
/// Fetch multiple URLs in parallel and return their bodies.
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

## License

MIT OR Apache-2.0
