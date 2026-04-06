---
name: spore-language
description: >
  Use for any task involving the Spore language, including writing, reviewing,
  debugging, or reasoning about Spore code, `.spore` files, `.sp` files,
  holes, and Spore program architecture.
---

# Spore Language Skill

## Mission

Spore is a language where **programmer intent is explicit, verifiable, and collaborative**.
Every function signature is a complete specification — types, errors, cost budget, and capabilities —
so humans and AI agents can collaborate through typed holes without ambiguity.

### Goals

- **Intent-first**: Signatures before implementations. Holes are first-class collaboration points.
- **Verifiable by construction**: Capabilities, costs, and effects are checked at compile time.
- **Supply-chain security**: Capability isolation ensures downloaded packages cannot access IO they don't declare.
- **AI-native workflow**: HoleReports provide self-contained context for AI code generation.

## Design philosophy — intent programming

Spore is built around one idea: **programmer intent should be explicit, verifiable, and collaborative**.

### Signatures are gravity centers

A function signature is a complete specification of intent — types, errors, cost budget, and required capabilities. The body can be a hole; the intent is already fully expressed.

```spore
fn fetch(url: String) -> String ! [NetError, Timeout]
    cost ≤ 1000
    uses [NetRead]
{
    ?todo
}
```

### Holes are collaboration points, not errors

A program with holes compiles successfully. Holes are how humans and Agents collaborate:

- **Human to Agent**: "Here is my intent (the signature). Fill this hole."
- **Agent to Human**: "Here is my proposal. Does it meet your intent?"

The compiler generates a self-contained HoleReport (JSON) for each hole: expected type, bindings, capabilities, cost budget, and candidate functions. No additional context is needed.

### Five-language heritage

| Language | Idea | How it shows up in Spore |
|----------|------|--------------------------|
| Agda | Holes | typed placeholders with compiler-provided context |
| Idris | Elaboration | Compiler infers details the programmer omits |
| Unison | Content-addressing | Modules identified by hash, not name+version |
| Elm | Error messages | Human-friendly diagnostics with repair hints |
| Roc | Managed effects | All IO through platform-provided effect handlers |

## The Spore programming workflow

**Follow these steps when building any Spore program.** This is the workflow Spore's tooling is designed to support.

### Step 1 — Define architecture (signatures + holes)

Write function signatures and types first. Use holes everywhere the implementation is not yet decided.

```spore
struct Order { id: Int, items: List[Item], total: Int }

fn calculate_total(items: List[Item]) -> Int cost ≤ O(n) { ?total_logic }
fn validate_order(order: Order) -> Order ! [ValidationError] { ?validation }
fn process_payment(order: Order) -> Receipt ! [PaymentError] uses [NetWrite] { ?payment }
```

### Step 2 — Verify the skeleton

Run `spore check` — the compiler validates signatures, capabilities, and cost budgets even with holes.

### Step 3 — Review hole reports

Run `spore holes <file>` to get the dependency graph and fill order. Start with leaf holes (no dependencies on other holes).

### Step 4 — Fill holes iteratively

- **Routine holes**: fill directly based on the HoleReport.
- **Design-critical holes**: stop and get an explicit human decision before proceeding. Do not bury important design choices in implementation.

### Step 5 — Re-check after each fill

Use `spore watch` for incremental re-checking on every save.

### Step 6 — Repeat until no holes remain

Zero holes = complete, fully verified program.

## Project structure

A Spore project follows a conventional layout managed by `spore.toml`:

```
my-app/
├── spore.toml          # project manifest
├── src/
│   ├── main.sp         # entry point (application)
│   └── billing/
│       ├── invoice.sp  # module: billing.invoice
│       └── types.sp    # module: billing.types
└── .gitignore
```

Cross-file imports use dot-separated module paths:

```spore
import billing.invoice
import billing.types
```

The compiler resolves modules from the `src/` directory and detects circular dependencies at compile time.

## CLI reference

```bash
# Compile and execute
spore run <file>                  # compile and execute (tree-walk interpreter)
spore run --json <file>           # output result as JSON

# Type-checking
spore check <file...>             # type-check one or more files
spore check --verbose <file>      # show inferred types, costs, capabilities
spore check --json <file>         # output diagnostics as JSON
spore check --deny-warnings <file> # treat warnings as errors

# Formatting
spore format <file>               # format source in-place (alias: spore fmt)
spore format --check <file>       # check if file is formatted (exit 1 if not)
spore format --diff <file>        # show formatting diff without writing

# Hole reports
spore holes <file>                # JSON hole report (types, bindings, candidates, DAG)

# Building
spore build <file>                # compile without executing (interpreter mode)

# Watch mode
spore watch <file>                # re-check on file changes
spore watch --json <file>         # NDJSON events for IDE/Agent consumption

# Project scaffolding (PR #26 — may not yet be on main)
spore new <name>                  # create new project directory
spore new <name> --type package   # project types: application (default), package, platform
spore init                        # initialize project in current directory
spore init --type package         # specify project type

# Version / help
spore --version                   # print version
spore help                        # show usage

# Development (building the compiler itself)
cargo build                       # build the compiler
cargo test --all                  # run all tests
```

## Language features

### Effect expressions — `perform` / `handle`

Spore supports algebraic effects via `perform` and `handle`. Effects are dispatched at runtime and checked at compile time via the `uses` clause.

### Cross-file import resolution

Multi-file compilation resolves `import billing.invoice` to `src/billing/invoice.sp`. The module dependency graph is validated for circular imports.

### LSP server

The `spore-lsp` binary provides IDE integration:

- **Completion** — context-aware suggestions for functions, types, and bindings.
- **Goto definition** — jump to function/type definitions.
- **Document symbols** — outline of structs, functions, and types in the current file.
- **Hover** — display type signatures and documentation for symbols.
- **Diagnostics** — real-time type errors, warnings, and cost analysis on save.

### Cost enforcement

Cost budgets declared in signatures are verified by the compiler. Functions whose cost cannot be determined structurally receive a warning. The `@unbounded` annotation opts out of cost checking (contagious to callers).

## Filling holes — checklist

When filling a `?name` hole:

1. Match the **expected_type** from the HoleReport.
2. Only use **bindings** listed in the report.
3. Stay within declared **capabilities** (`uses` clause).
4. Handle all **errors** in `errors_to_handle`.
5. Respect the **cost budget** if declared.
6. Follow the dependency graph: fill leaf holes first, then work upward.

## Key builtins

List: `len`, `head`, `tail`, `append`, `prepend`, `reverse`, `range`, `contains`.
Higher-order: `map`, `filter`, `fold`, `each`.
String: `string_length`, `split`, `trim`, `to_upper`, `to_lower`, `starts_with`, `ends_with`, `replace`.
Math: `abs`, `min`, `max`, `to_string`.
IO: `print`, `println`, `read_line`.
Operators: `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`, `!`, `|>`, `?`.

## Current limitations

1. No loops — use recursion or higher-order functions.
2. Concurrency syntax-only — `spawn`/`await`/`parallel_scope` parse but do not execute.
3. Tree-walking interpreter — no compiled native output yet.
4. Refinement types — parsed but not enforced at type-check time.
