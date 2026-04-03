---
name: spore-language
description: >
  Use for any task involving the Spore language, including writing, reviewing,
  debugging, or reasoning about Spore code, `.spore` files, `.sp` files,
  holes, and Spore program architecture.
---

# Spore Language Skill

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

## CLI reference

```bash
spore check <file...>        # type-check only
spore check --verbose <file> # show inferred types, costs, capabilities
spore holes <file>           # JSON hole report (types, bindings, candidates, DAG)
spore run <file>             # compile and execute (tree-walk interpreter)
spore watch <file>           # re-check on file changes
spore watch --json <file>    # NDJSON events for IDE/Agent consumption
cargo build                  # build the compiler
cargo test --all             # run all tests
```

## Syntax essentials

### Core rules

- Expression-only: no statements, no loops, no null.
- Fixed signature clause order: `-> return` then `! errors` then `where` then `cost` then `uses` then `body`.
- Semicolons: with semicolon = statement (value discarded); without = return expression.
- No custom operators. Pipe `|>` and try `?` are built in.

### Data types

```spore
struct Point { x: Int, y: Int }

type Shape { Circle(Int), Rect(Int, Int) }

fn area(s: Shape) -> Int {
    match s {
        Circle(r) => r * r * 3,
        Rect(w, h) => w * h,
    }
}
```

Pattern matching is exhaustive. Supported: variable, wildcard `_`, literal, constructor, struct, or-pattern `A | B`, list `[h, ..t]`.

### Primitives

`Int`, `Float`, `Bool`, `String`, `Char`, `Unit`, `Never`.

Composite: `List[T]`, `Option[T]`, `Result[T, E]`, `(A) -> B`, `Task[T]`.

### Capabilities

Declare effects with `uses`. Callee capabilities must be a subset of caller capabilities.

```spore
fn read_data(path: String) -> String uses [FileRead] { ?todo }
fn transform(data: String) -> String { data |> trim }  // pure — no uses
```

Atomic capabilities: `Compute`, `FileRead`, `FileWrite`, `NetRead`, `NetWrite`, `StateRead`, `StateWrite`, `Spawn`, `Clock`, `Random`, `Exit`.

### Cost analysis

```spore
fn search(list: List[Int], target: Int) -> Bool cost ≤ O(n) { ?search }
```

Three tiers: automatic structural recursion (~70%), declared `cost ≤ expr` (~20%), `@unbounded` escape (~10%, contagious).

### Error sets

```spore
fn load(path: String) -> Config ! [IoError, ParseError] uses [FileRead] {
    let raw = read_file(path)?;   // ? propagates errors
    parse_config(raw)?
}
```

### Lambdas and pipes

```spore
let double = |x: Int| x * 2;
numbers |> filter(|x: Int| x > 0) |> map(|x: Int| x * 2) |> fold(0, |a: Int, b: Int| a + b)
```

### Traits (capabilities)

```spore
capability Display[T] { fn show(self: T) -> String }
impl Display for Point { fn show(self: Point) -> String { to_string(self.x) + ", " + to_string(self.y) } }
```

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
