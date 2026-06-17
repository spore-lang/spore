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
Every function signature is a complete specification — types, errors, effects,
realization-shape budgets, and properties — so humans and Agents can collaborate
through typed holes without ambiguity.

### Goals

- **Intent-first**: signatures before implementations; holes are first-class collaboration points.
- **Verifiable by construction**: required effects, budgets, properties, and error contracts are checked at compile time.
- **Supply-chain security**: effect isolation ensures downloaded packages cannot access IO they do not declare.
- **Agent-native workflow**: HoleReports provide self-contained context for code generation and review.

## Design philosophy — intent programming

Spore is built around one idea: **programmer intent should be explicit, verifiable, and collaborative**.

### Signatures are gravity centers

A function signature is a complete specification of intent. The body can be a
hole; the intent is already fully expressed.

```spore
effect NetConnect {
    fn fetch(url: Str) -> Str ! NetError | Timeout;
}

fn fetch(url: Str) -> Str ! NetError | Timeout
    uses [NetConnect]
    budget { calls: 1 }
    properties {
        input_has_length(url: Str): len(url) >= 0
    }
{
    ?todo
}
```

### Holes are collaboration points, not errors

A program with holes compiles successfully. Holes are how humans and Agents collaborate:

- **Human to Agent**: "Here is my intent (the signature). Fill this hole."
- **Agent to Human**: "Here is my proposal. Does it meet your intent?"

The compiler generates a self-contained HoleReport for each hole: expected type,
visible bindings, available effects, budget context, property context, and
candidate functions. No additional context is needed.

### Five-language heritage

| Language | Idea               | How it shows up in Spore                          |
| -------- | ------------------ | ------------------------------------------------- |
| Agda     | Holes              | typed placeholders with compiler-provided context |
| Idris    | Elaboration        | Compiler infers details the programmer omits      |
| Unison   | Content-addressing | Modules identified by content hash                |
| Elm      | Error messages     | Human-friendly diagnostics with repair hints      |
| Roc      | Managed effects    | All IO through platform-provided effect handlers  |

## The Spore programming workflow

**Follow these steps when building any Spore program.** This is the workflow Spore's tooling is designed to support.

### Step 1 — Define architecture (signatures + holes)

Write function signatures and types first. Use holes everywhere the implementation is not yet decided.

```spore
struct Order { id: I32, items: List[Item], total: I32 }

fn calculate_total(items: List[Item]) -> I32 { ?total_logic }
fn validate_order(order: Order) -> Order ! ValidationError { ?validation }
fn process_payment(order: Order) -> Receipt ! PaymentError
    uses [NetConnect]
    budget { calls: 2 }
    properties {
        has_receipt(order: Order): true
    }
{
    ?payment
}
```

### Step 2 — Verify the skeleton

Run `spore check` — the compiler validates signatures, `uses` clauses, budgets,
properties, and error contracts even with holes. Hole-bearing programs are useful
for `check` and `holes`, while `run` stops if execution reaches an unfilled hole.

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

```text
my-app/
├── spore.toml          # project manifest
├── src/
│   ├── main.sp         # default entry module (application)
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

At the project layer, an `entry` selects a source path/module. The selected entry
module then provides a startup function that must satisfy the configured
Platform's startup contract.

## CLI reference

```bash
# Compile and execute
spore run <file>                   # compile and execute (tree-walk interpreter)
spore run --json <file>            # output result as JSON

# Type-checking
spore check <file...>              # type-check one or more files
spore check --verbose <file>       # show detailed compiler info
spore check --json <file>          # output diagnostics as JSON
spore check --deny-warnings <file> # treat warnings as errors

# Property validation
spore test <file...>               # validate source properties

# Formatting
spore format <file>                # format source in-place (alias: spore fmt)
spore format --check <file>        # check if file is formatted
spore format --diff <file>         # show formatting diff without writing

# Hole reports
spore holes <file>                 # JSON hole report

# Building
spore build <file>                 # compile a standalone scalar .sp file to a native .o object

# Watch mode
spore watch <file>                 # re-check on file changes
spore watch --json <file>          # NDJSON events for IDE/Agent consumption

# Project scaffolding
spore new <name>                   # create new project directory
spore new <name> --type package    # project types: application, package, platform
spore init                         # initialize project in current directory
spore init --type package          # specify project type

# Version
spore --version                    # print version

# Development
cargo build                        # build the compiler
cargo test --all                   # run all tests
```

## Language features

### Effect expressions — `perform` / `handle`

Spore supports algebraic effects via `perform` and `handle`. Effects are
dispatched at runtime and checked at compile time via the `uses` clause.

### Intent signatures

Use `uses`, `budget`, and `properties` after the Base Signature and before the body:

```spore
fn dedupe[T: Eq + Hash](xs: List[T]) -> List[T]
budget {
    branches: 3
    nesting: 2
    holes: 0
}
properties {
    idempotent(xs: List[T]): dedupe(dedupe(xs)) == dedupe(xs)
}
{
    ?dedupe_body
}
```

### Cross-file import resolution

Multi-file compilation resolves `import billing.invoice` to `src/billing/invoice.sp`. The module dependency graph is validated for circular imports.

### LSP server

The `spore-lsp` binary provides IDE integration:

- **Completion** — context-aware suggestions for functions, types, and bindings.
- **Goto definition** — jump to function/type definitions.
- **Document symbols** — outline of structs, functions, and types in the current file.
- **Hover** — display type signatures and documentation for symbols.
- **Diagnostics** — compiler diagnostics on open/change/save.

### Budget enforcement

Use `budget { field: limit }` for source-level realization-shape constraints.
Built-in fields include `branches`, `nesting`, `recursion`, `parallelism`,
`calls`, `effects`, and `holes`.

## Filling holes — checklist

When filling a `?name` hole:

1. Match the **expected_type** from the HoleReport.
2. Only use **bindings** listed in the report.
3. Stay within declared **required effects** (`uses` clause).
4. Handle all **errors** in `errors_to_handle`.
5. Respect declared **budget_context** and **property_context**.
6. Follow the dependency graph: fill leaf holes first, then work upward.

## Key builtins

List: `len`, `head`, `tail`, `append`, `prepend`, `reverse`, `range`, `contains`.
Higher-order: `map`, `filter`, `fold`, `each`.
Str/text: `string_length`, `split`, `trim`, `to_upper`, `to_lower`, `starts_with`, `ends_with`, `replace`.
Math: `abs`, `min`, `max`, `to_string`.
IO: `print`, `println`, `read_line`.
Operators: `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`, `!`, `|>`, `?`.

## Current limitations

1. No loops — use recursion or higher-order functions.
2. Unfilled holes are checkable/reportable, but execution stops with a runtime error if a hole is reached.
3. Structured concurrency is only the current subset (`parallel_scope`, `spawn`, `.await`, `select`).
4. Runtime execution still uses the tree-walking interpreter; `spore build` emits experimental native `.o` artifacts for standalone scalar files.
5. Refinement types are parsed, with enforcement still expanding through the checker and evidence pipeline.
