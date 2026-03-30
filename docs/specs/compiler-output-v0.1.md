# Spore Compiler Output Format — Design Document v0.1

> **Status**: Draft
> **Scope**: `sporec` compiler, LSP server, CI/CD pipelines, Agent workflow
> **Depends on**: Signature system, Cost system, Capability system, Hole system, Module system, Snapshot system

---

## 0. Guiding Principles

| Principle | Implication |
|---|---|
| **Output is communication** | Every diagnostic is a conversation with the developer (or Agent). It must answer: *what went wrong*, *where*, *why*, and *how to fix it*. |
| **JSON is the superset** | `--json` contains every piece of information the compiler knows about a diagnostic. Default and `--verbose` are progressively detailed human renderings of the same underlying data. |
| **Every diagnostic is actionable** | Every error, warning, and note includes a `help:` line with a concrete fix suggestion. No diagnostic leaves the reader without a next step. |
| **Three modes, not levels** | Default, `--verbose`, and `--json` are format choices selected by flags — not numbered levels of detail. They serve different audiences and workflows. |
| **Compiler-first** | All diagnostic information is available via `sporec` CLI output — no IDE required. LSP clients consume `--json`; humans read default or `--verbose`. |

---

## 1. Overview

### 1.1 Philosophy

Compiler output is the primary interface between the compiler and its users. In Spore, "users" include human developers, CI/CD systems, LSP clients, and Agents. Good compiler output must serve all of them without compromise.

The design draws inspiration from Rust's error reporting (concise, pointer-based, color-coded) while incorporating Spore-specific concepts: capabilities, cost budgets, holes, and snapshot hashes. The result is a diagnostic system where every message is self-contained and actionable.

### 1.2 Three Output Modes

The compiler supports three output modes, selected by CLI flags:

| Mode | Flag | Audience | Content |
|------|------|----------|---------|
| **Default** | *(none)* | All developers, Agents | Rust-style concise text with color coding and help suggestions |
| **Verbose** | `--verbose` | Developers debugging complex errors, Agents doing deep analysis | Default + inference chains + candidate types + capability/cost context |
| **JSON** | `--json` | CI/CD, scripts, LSP clients, machine consumers | LSP-compatible JSON with Spore extensions — the superset of all information |

Key relationship: `Default ⊂ --verbose ⊂ --json`. JSON contains everything. Verbose adds analysis detail to default. Default is the minimal actionable view.

### 1.3 Error Code System

All diagnostics carry a categorized error code:

| Prefix | Category | Examples |
|--------|----------|----------|
| `E0xxx` | Type errors | type mismatch, missing field, arity error |
| `W0xxx` | Warnings | unused variable, redundant pattern, shadowing |
| `C0xxx` | Capability violations | undeclared capability, exceeding module ceiling |
| `K0xxx` | Cost violations | exceeding budget, unbounded call without `cost_limit` |
| `H0xxx` | Hole diagnostics | hole report, partial function, hole type conflict |
| `M0xxx` | Module errors | circular dependency, visibility violation, import not found |

Every code is queryable: `sporec --explain E0301` opens a detailed explanation.

---

## 2. Default Output Format

### 2.1 Layout Anatomy

A default diagnostic consists of these visual elements:

```
<severity>[<code>]: <headline message>
  --> <file>:<line>:<col>
   |
<line> | <source line>
   |    <underline> <inline note>
   |
   = note: <additional context>
help: <suggested fix>
```

Components:
- **Severity tag**: `error`, `warning`, `note` — color-coded (see §2.2)
- **Error code**: bracketed, e.g. `[E0301]`
- **Headline**: one-line summary of the problem
- **Location pointer**: `-->` followed by `file:line:col`
- **Gutter**: `|` column for source display
- **Source line**: the offending line with line number
- **Underline**: `^^^` under the relevant span, with inline note
- **Notes**: additional context lines prefixed with `= note:`
- **Help**: always present, prefixed with `help:`

### 2.2 Color Scheme

| Element | Color | ANSI Code |
|---------|-------|-----------|
| `error` + error code | Red (bold) | `\x1b[1;31m` |
| `warning` + warning code | Yellow (bold) | `\x1b[1;33m` |
| `note` | Blue (bold) | `\x1b[1;34m` |
| `help` | Green (bold) | `\x1b[1;32m` |
| Line numbers | Blue | `\x1b[34m` |
| Source text | Default | — |
| Underline (`^^^`) | Matches severity color | — |

Colors are disabled when output is piped (not a TTY) or when `--no-color` is passed. The `--json` mode never includes ANSI codes.

### 2.3 Multi-Line Errors

When an error spans multiple lines, the gutter marks the full range:

```
error[E0102]: struct missing required field `currency`
  --> src/billing.spore:15:5
   |
15 | /   let invoice = Invoice {
16 | |       amount: 100,
17 | |       recipient: user,
18 | |   }
   | |___^ missing field `currency`
   |
help: add the missing field: `currency: Currency.USD`
```

### 2.4 Multi-Error Output

Errors are separated by blank lines. A summary line appears at the end:

```
error[E0301]: type mismatch
  --> src/billing.spore:42:22
   ...

warning[W0101]: unused variable `temp`
  --> src/billing.spore:38:9
   ...

error: aborting due to 1 error; 1 warning emitted
```

The exit code is non-zero if any errors (not just warnings) are present.

### 2.5 Examples by Category

#### Type Error (E0301)

```
error[E0301]: type mismatch
  --> src/billing.spore:42:22
   |
42 |     charge(card, "fifty dollars")
   |                  ^^^^^^^^^^^^^^^ expected `Money`, found `String`
   |
help: try `Money.from_string("fifty dollars")`
```

#### Warning (W0101)

```
warning[W0101]: unused variable
  --> src/billing.spore:38:9
   |
38 |     let temp = calculate_subtotal(items)
   |         ^^^^ `temp` is never read
   |
help: prefix with underscore if intentional: `_temp`
```

#### Capability Violation (C0101)

```
error[C0101]: undeclared capability
  --> src/report.spore:27:5
   |
27 |     http.get(endpoint)
   |     ^^^^^^^^^^^^^^^^^^ requires `NetRead`, not declared in `uses`
   |
   = note: enclosing function `fetch_data` has `uses []`
   = note: module ceiling for `report` allows [Compute, FileRead]
help: add a `uses [NetRead]` clause to `fetch_data`
```

#### Cost Violation (K0101)

```
error[K0101]: cost budget exceeded
  --> src/analytics.spore:55:5
   |
55 |     full_table_scan(records)
   |     ^^^^^^^^^^^^^^^^^^^^^^^^ this call costs 8200 op
   |
   = note: `summarize` budget is 5000 op, already used 1200 op
   = note: remaining budget: 3800 op, shortfall: 4400 op
help: filter `records` before scanning, or increase budget to `cost ≤ 10000`
```

#### Hole Diagnostic (H0101)

```
note[H0101]: hole `tax_logic` requires filling
  --> src/tax.spore:12:5
   |
12 |     ?tax_logic
   |     ^^^^^^^^^^ expected type: `Money`, remaining cost budget: 400 op
   |
   = note: available bindings: income: Money, region: Region
   = note: available capabilities: [TaxTable]
   = note: candidate: `tax_table.lookup(region, income) -> Money`
help: run `sporec --query-hole tax_logic` for full HoleReport
```

#### Module Error (M0101)

```
error[M0101]: circular module dependency
  --> src/auth.spore:3:1
   |
 3 | use billing.invoice
   | ^^^^^^^^^^^^^^^^^^^ `auth` imports `billing.invoice`
   |
   = note: cycle: auth -> billing.invoice -> auth
   = note: detected during dependency resolution
help: extract shared types into a new module (e.g. `common.types`)
```

---

## 3. --verbose Output Format

### 3.1 What Verbose Adds

The `--verbose` flag appends additional analysis blocks after each diagnostic. These blocks are indented and visually distinct:

- **Inference chain**: step-by-step type derivation showing how the compiler reached its conclusion
- **Candidates considered**: possible conversions, overloads, or functions the compiler evaluated
- **Capability context**: which capabilities are in scope at the error site
- **Cost context**: cost used so far vs. budget at the error site

### 3.2 Verbose Block Format

Verbose blocks appear immediately after the `help:` line, indented with two extra spaces:

```
  inference chain:
    <expr> : <Type>       (<source>)
    <expr> : <Type>       (<source>)
    <Type> ≠ <Type>       (<reason>)

  candidates considered:
    <From> -> <To> via <path>     <status>

  capability context: [<Cap1>, <Cap2>, ...]
  cost at this point: <used> / budget <total>
```

### 3.3 Verbose Examples

#### Type Error (E0301) — Verbose

```
error[E0301]: type mismatch
  --> src/billing.spore:42:22
   |
42 |     charge(card, "fifty dollars")
   |                  ^^^^^^^^^^^^^^^ expected `Money`, found `String`
   |
help: try `Money.from_string("fifty dollars")`

  inference chain:
    "fifty dollars" : String       (literal, line 42)
    charge.amount : Money          (from signature, sig@b3c1e2)
    String ≠ Money                 (nominal mismatch)

  candidates considered:
    String -> Money via Money.from_string  ✓ (exact match)
    String -> Money via Money.parse        ✓ (may raise ParseError)

  capability context: [PaymentGateway]
  cost at this point: 120 / budget 500
```

#### Capability Violation (C0101) — Verbose

```
error[C0101]: undeclared capability
  --> src/report.spore:27:5
   |
27 |     http.get(endpoint)
   |     ^^^^^^^^^^^^^^^^^^ requires `NetRead`, not declared in `uses`
   |
   = note: enclosing function `fetch_data` has `uses []`
   = note: module ceiling for `report` allows [Compute, FileRead]
help: add a `uses [NetRead]` clause to `fetch_data`

  inference chain:
    http.get : Fn(Url) -> Response ! [NetworkError]   (from module http, sig@e4a1f9)
    http.get.uses : [NetRead]                         (from signature)
    fetch_data.uses : []                              (declared)
    [NetRead] ⊄ []                                    (capability not available)

  candidates considered:
    (none — no alternative without NetRead)

  capability context: [] (function level), [Compute, FileRead] (module ceiling)
  cost at this point: 45 / budget 2000
```

#### Cost Violation (K0101) — Verbose

```
error[K0101]: cost budget exceeded
  --> src/analytics.spore:55:5
   |
55 |     full_table_scan(records)
   |     ^^^^^^^^^^^^^^^^^^^^^^^^ this call costs 8200 op
   |
   = note: `summarize` budget is 5000 op, already used 1200 op
   = note: remaining budget: 3800 op, shortfall: 4400 op
help: filter `records` before scanning, or increase budget to `cost ≤ 10000`

  inference chain:
    full_table_scan.cost : 8200 op        (from signature, sig@f7b2c3)
    summarize.cost_budget : 5000 op       (declared)
    prior statements cost : 1200 op       (accumulated)
    1200 + 8200 = 9400 > 5000             (budget exceeded)

  cost breakdown (summarize):
    line 50: let filtered = preprocess(records)   → 800 op
    line 52: let stats = basic_stats(filtered)    → 400 op
    line 55: full_table_scan(records)             → 8200 op  ← violation
    total: 9400 / budget 5000

  capability context: [Compute, Module<analytics>]
  cost at this point: 1200 / budget 5000
```

#### Hole Diagnostic (H0101) — Verbose

```
note[H0101]: hole `tax_logic` requires filling
  --> src/tax.spore:12:5
   |
12 |     ?tax_logic
   |     ^^^^^^^^^^ expected type: `Money`, remaining cost budget: 400 op
   |
   = note: available bindings: income: Money, region: Region
   = note: available capabilities: [TaxTable]
   = note: candidate: `tax_table.lookup(region, income) -> Money`
help: run `sporec --query-hole tax_logic` for full HoleReport

  inference chain:
    ?tax_logic : Money            (inferred from return type of calculate_tax)
    cost_consumed : 100 op        (prior to hole)
    cost_remaining : 400 op       (500 - 100)

  candidates considered:
    tax_table.lookup(region, income) -> Money    ✓ (cost 50 op, within budget)
    tax_table.calculate(income, region) -> Money ! [InvalidRegion]    ✓ (cost 200 op, error type compatible)

  capability context: [TaxTable]
  cost at this point: 100 / budget 500
```

#### Module Error (M0201) — Verbose

```
error[M0201]: visibility violation
  --> src/api.spore:14:12
   |
14 |     let key = auth.secret_key
   |               ^^^^^^^^^^^^^^^ `secret_key` is private to module `auth`
   |
   = note: `secret_key` is declared without `pub` in `auth`
help: use the public accessor `auth.get_key(token)` instead

  inference chain:
    auth.secret_key : ApiKey           (from module auth, sig@c2d4e6)
    auth.secret_key.visibility : private  (module-private, no pub)
    api → auth : uses relationship     (import on line 2)
    private ≠ accessible               (visibility violation)

  candidates considered:
    auth.get_key(token: AuthToken) -> ApiKey    ✓ (public, cost 5 op)
    auth.derive_key(seed: Seed) -> ApiKey       ✓ (pub(pkg), same package)

  capability context: [Compute, NetRead]
  cost at this point: 30 / budget 1000
```

---

## 4. --json Output Format

### 4.1 Design Principle

The `--json` output is the single source of truth. Every field rendered in default or verbose mode is derived from a JSON field. LSP clients, CI/CD pipelines, scripts, and Agents consume this format directly.

### 4.2 JSON Schema

A complete `--json` output is a JSON object containing an array of diagnostics and a summary:

```json
{
  "version": "0.1",
  "compiler": "sporec",
  "diagnostics": [ <Diagnostic>, ... ],
  "summary": {
    "errors": 2,
    "warnings": 1,
    "notes": 3,
    "holes": 1
  }
}
```

### 4.3 Diagnostic Object Schema

Each diagnostic follows this schema:

```json
{
  "severity": "error" | "warning" | "note",
  "code": "<string>",
  "message": "<string>",
  "location": {
    "file": "<string>",
    "range": {
      "start": { "line": "<int>", "col": "<int>" },
      "end": { "line": "<int>", "col": "<int>" }
    }
  },
  "related": [
    {
      "location": { "file": "<string>", "range": { ... } },
      "message": "<string>"
    }
  ],
  "inference_chain": [
    {
      "expr": "<string>",
      "type": "<string>",
      "source": "<string>"
    }
  ],
  "candidates": [
    {
      "from": "<string>",
      "to": "<string>",
      "via": "<string>",
      "quality": "exact" | "partial" | "incompatible",
      "note": "<string> | null"
    }
  ],
  "context": {
    "capabilities": ["<string>"],
    "cost_used": "<int> | null",
    "cost_budget": "<int> | null",
    "enclosing_function": "<string>",
    "enclosing_module": "<string>",
    "hole": "<string> | null"
  },
  "suggested_fix": {
    "description": "<string>",
    "applicability": "safe" | "unsafe" | "informational",
    "edits": [
      {
        "file": "<string>",
        "range": {
          "start": { "line": "<int>", "col": "<int>" },
          "end": { "line": "<int>", "col": "<int>" }
        },
        "new_text": "<string>"
      }
    ]
  }
}
```

### 4.4 Field Descriptions

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `severity` | string | yes | One of `"error"`, `"warning"`, `"note"` |
| `code` | string | yes | Error code, e.g. `"E0301"` |
| `message` | string | yes | Human-readable headline |
| `location` | object | yes | Primary source location |
| `related` | array | no | Related source locations with messages |
| `inference_chain` | array | no | Step-by-step type derivation |
| `candidates` | array | no | Conversions/overloads the compiler considered |
| `context` | object | yes | Capability, cost, and scope context |
| `suggested_fix` | object | no | Machine-applicable fix suggestion |

### 4.5 LSP Diagnostic Compatibility

The JSON schema is designed for zero-friction LSP integration. The mapping:

| Spore JSON Field | LSP `Diagnostic` Field | Notes |
|------------------|----------------------|-------|
| `severity` | `severity` | Direct map: `"error"` → 1, `"warning"` → 2, `"note"` → 3 |
| `code` | `code` | Direct map |
| `message` | `message` | Direct map |
| `location.range` | `range` | Direct map (0-indexed in LSP, 1-indexed in Spore; LSP adapter adjusts) |
| `related` | `relatedInformation` | Direct map |
| `suggested_fix` | `codeAction` | Mapped to LSP `CodeAction` with `edit.changes` |

Spore extension fields (`inference_chain`, `candidates`, `context`) are placed in the standard `data` field for LSP consumers that understand Spore:

```json
{
  "severity": 1,
  "code": "E0301",
  "message": "type mismatch: expected `Money`, found `String`",
  "range": { ... },
  "relatedInformation": [ ... ],
  "data": {
    "spore": {
      "inference_chain": [ ... ],
      "candidates": [ ... ],
      "context": { ... }
    }
  }
}
```

### 4.6 Batch and Streaming Output

**Batch mode** (default): The compiler emits a single JSON object after compilation completes. All diagnostics are in the `diagnostics` array.

**Streaming mode** (`--json --stream`): The compiler emits one JSON object per diagnostic as it is discovered, using newline-delimited JSON (NDJSON). Each line is a valid `Diagnostic` object. The summary is emitted as a final line with `"type": "summary"`.

```
{"severity":"error","code":"E0301","message":"type mismatch",...}
{"severity":"warning","code":"W0101","message":"unused variable",...}
{"type":"summary","errors":1,"warnings":1,"notes":0,"holes":0}
```

Streaming mode is preferred for large projects and CI/CD pipelines where early feedback matters.

### 4.7 Full JSON Examples

#### Type Error (E0301) — JSON

```json
{
  "severity": "error",
  "code": "E0301",
  "message": "type mismatch: expected `Money`, found `String`",
  "location": {
    "file": "src/billing.spore",
    "range": {
      "start": { "line": 42, "col": 22 },
      "end": { "line": 42, "col": 37 }
    }
  },
  "related": [
    {
      "location": {
        "file": "src/billing.spore",
        "range": { "start": { "line": 10, "col": 5 }, "end": { "line": 10, "col": 20 } }
      },
      "message": "charge.amount declared as Money here"
    }
  ],
  "inference_chain": [
    { "expr": "\"fifty dollars\"", "type": "String", "source": "literal" },
    { "expr": "charge.amount", "type": "Money", "source": "signature:b3c1e2" }
  ],
  "candidates": [
    { "from": "String", "to": "Money", "via": "Money.from_string", "quality": "exact", "note": null },
    { "from": "String", "to": "Money", "via": "Money.parse", "quality": "partial", "note": "may raise ParseError" }
  ],
  "context": {
    "capabilities": ["PaymentGateway"],
    "cost_used": 120,
    "cost_budget": 500,
    "enclosing_function": "charge_customer",
    "enclosing_module": "billing.payment",
    "hole": null
  },
  "suggested_fix": {
    "description": "use Money.from_string",
    "applicability": "safe",
    "edits": [
      {
        "file": "src/billing.spore",
        "range": { "start": { "line": 42, "col": 22 }, "end": { "line": 42, "col": 37 } },
        "new_text": "Money.from_string(\"fifty dollars\")"
      }
    ]
  }
}
```

#### Cost Violation (K0101) — JSON

```json
{
  "severity": "error",
  "code": "K0101",
  "message": "cost budget exceeded: `full_table_scan` costs 8200 op, budget remaining 3800 op",
  "location": {
    "file": "src/analytics.spore",
    "range": {
      "start": { "line": 55, "col": 5 },
      "end": { "line": 55, "col": 29 }
    }
  },
  "related": [
    {
      "location": {
        "file": "src/analytics.spore",
        "range": { "start": { "line": 48, "col": 1 }, "end": { "line": 48, "col": 15 } }
      },
      "message": "`summarize` declares `cost ≤ 5000`"
    }
  ],
  "inference_chain": [
    { "expr": "full_table_scan", "type": "cost:8200", "source": "signature:f7b2c3" },
    { "expr": "summarize.budget", "type": "cost:5000", "source": "declared" },
    { "expr": "prior_cost", "type": "cost:1200", "source": "accumulated" }
  ],
  "candidates": [],
  "context": {
    "capabilities": ["Compute", "Module<analytics>"],
    "cost_used": 1200,
    "cost_budget": 5000,
    "enclosing_function": "summarize",
    "enclosing_module": "analytics.reports",
    "hole": null
  },
  "suggested_fix": {
    "description": "increase budget or filter input",
    "applicability": "informational",
    "edits": []
  }
}
```

#### Hole Diagnostic (H0101) — JSON

```json
{
  "severity": "note",
  "code": "H0101",
  "message": "hole `tax_logic` requires filling: expected type `Money`",
  "location": {
    "file": "src/tax.spore",
    "range": {
      "start": { "line": 12, "col": 5 },
      "end": { "line": 12, "col": 15 }
    }
  },
  "related": [],
  "inference_chain": [
    { "expr": "?tax_logic", "type": "Money", "source": "inferred:return_type" },
    { "expr": "cost_remaining", "type": "cost:400", "source": "budget:500-consumed:100" }
  ],
  "candidates": [
    { "from": "hole", "to": "Money", "via": "tax_table.lookup(region, income)", "quality": "exact", "note": "cost 50 op" },
    { "from": "hole", "to": "Money", "via": "tax_table.calculate(income, region)", "quality": "exact", "note": "cost 200 op, raises InvalidRegion" }
  ],
  "context": {
    "capabilities": ["TaxTable"],
    "cost_used": 100,
    "cost_budget": 500,
    "enclosing_function": "calculate_tax",
    "enclosing_module": "tax.calculator",
    "hole": "tax_logic"
  },
  "suggested_fix": {
    "description": "fill with tax_table.lookup",
    "applicability": "safe",
    "edits": [
      {
        "file": "src/tax.spore",
        "range": { "start": { "line": 12, "col": 5 }, "end": { "line": 12, "col": 15 } },
        "new_text": "tax_table.lookup(region, income)"
      }
    ]
  }
}
```

---

## 5. Error Code Registry

### 5.1 E0xxx — Type Errors

Type errors arise from mismatches in the Spore type system: wrong types, missing fields, arity violations, and generic constraint failures.

| Code | Name | Description |
|------|------|-------------|
| `E0101` | missing-field | Struct literal missing a required field |
| `E0102` | unknown-field | Struct literal contains a field not in the type definition |
| `E0201` | arity-mismatch | Function called with wrong number of arguments |
| `E0202` | named-arg-mismatch | Named argument does not match any parameter |
| `E0301` | type-mismatch | Expression has a type incompatible with the expected type |
| `E0302` | return-type-mismatch | Function body returns a type different from declared return type |
| `E0303` | error-type-mismatch | Function raises an error type not declared in `!` list |
| `E0401` | constraint-not-satisfied | Generic type argument does not satisfy trait constraint |
| `E0402` | ambiguous-type | Type inference cannot determine a unique type |
| `E0501` | pattern-exhaustiveness | Match expression does not cover all variants |

#### E0101 — missing-field

```
error[E0101]: struct missing required field `currency`
  --> src/billing.spore:15:5
   |
15 | /   let invoice = Invoice {
16 | |       amount: 100,
17 | |       recipient: user,
18 | |   }
   | |___^ missing field `currency`
   |
   = note: `Invoice` requires fields: amount, recipient, currency
help: add the missing field: `currency: Currency.USD`
```

#### E0201 — arity-mismatch

```
error[E0201]: function `format_date` takes 2 arguments, but 3 were given
  --> src/report.spore:22:5
   |
22 |     format_date(date, "YYYY-MM-DD", locale)
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ expected 2 arguments
   |
   = note: signature: fn format_date(date: Date, fmt: String) -> String
help: remove the extra argument `locale`
```

#### E0401 — constraint-not-satisfied

```
error[E0401]: type `Event` does not satisfy constraint `Serialize`
  --> src/api.spore:33:15
   |
33 |     serialize(event)
   |               ^^^^^ `Event` does not implement `Serialize`
   |
   = note: `serialize` requires `T: Serialize` (from where clause)
   = note: `Event` implements: [Eq, Display]
help: derive `Serialize` for `Event` or implement it manually
```

#### E0501 — pattern-exhaustiveness

```
error[E0501]: non-exhaustive match: `Pending` not covered
  --> src/order.spore:60:5
   |
60 | /   match status {
61 | |       Active => handle_active()
62 | |       Cancelled => handle_cancelled()
63 | |   }
   | |___^ missing arm for `Pending`
   |
   = note: `OrderStatus` has variants: Active, Pending, Cancelled
help: add `Pending => ...` arm or use `_ => ...` as catch-all
```

### 5.2 W0xxx — Warnings

Warnings indicate code that compiles but may indicate mistakes or suboptimal patterns.

| Code | Name | Description |
|------|------|-------------|
| `W0101` | unused-variable | Variable is bound but never read |
| `W0102` | unused-import | Module import is never referenced |
| `W0103` | unused-function | Private function is never called |
| `W0201` | redundant-pattern | Match arm is unreachable due to prior arm |
| `W0202` | redundant-constraint | Generic constraint is implied by another |
| `W0301` | shadowing | Variable shadows an existing binding in outer scope |
| `W0302` | implicit-discard | Expression result is discarded without explicit `_` |
| `W0401` | deprecated-usage | Function or type is marked as deprecated |

#### W0201 — redundant-pattern

```
warning[W0201]: redundant match arm
  --> src/parser.spore:44:9
   |
42 |     match token {
43 |         Number(n) => parse_number(n)
44 |         Number(_) => unreachable()
   |         ^^^^^^^^^ this arm is unreachable — already covered by line 43
   |
help: remove the redundant arm
```

#### W0301 — shadowing

```
warning[W0301]: variable `count` shadows existing binding
  --> src/stats.spore:18:9
   |
15 |     let count = items.len()
   |         ----- first binding here
   ...
18 |     let count = filtered.len()
   |         ^^^^^ shadows previous `count`
   |
help: rename to `filtered_count` to avoid ambiguity
```

#### W0401 — deprecated-usage

```
warning[W0401]: `old_format` is deprecated
  --> src/export.spore:30:12
   |
30 |     let s = old_format(data)
   |             ^^^^^^^^^^ deprecated since v0.3
   |
   = note: use `format_v2` instead (see migration guide)
help: replace with `format_v2(data)`
```

### 5.3 C0xxx — Capability Violations

Capability errors occur when code tries to perform operations beyond its declared capability set.

| Code | Name | Description |
|------|------|-------------|
| `C0101` | undeclared-capability | Function uses a capability not listed in `uses` |
| `C0102` | exceeds-ceiling | Function declares a capability its module ceiling does not allow |
| `C0103` | callee-capability-leak | Calling a function whose `uses` exceeds the caller's `uses` |
| `C0201` | platform-capability-denied | Package requests a capability the Platform does not grant |
| `C0301` | effects-capability-conflict | Declared effects conflict with declared capabilities |

#### C0102 — exceeds-ceiling

```
error[C0102]: capability exceeds module ceiling
  --> src/report.spore:8:5
   |
 8 |     uses [NetWrite, Compute]
   |           ^^^^^^^^ `NetWrite` exceeds module ceiling
   |
   = note: module `report` has ceiling: [Compute, FileRead, NetRead]
   = note: `NetWrite` is not in the ceiling
help: either add `NetWrite` to the module ceiling or move this function to a module that allows it
```

#### C0103 — callee-capability-leak

```
error[C0103]: callee requires capabilities not available to caller
  --> src/handler.spore:25:5
   |
25 |     send_email(user, message)
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^ `send_email` requires [NetWrite, Module<smtp>]
   |
   = note: `process_request` declares `uses [Compute, NetRead]`
   = note: missing capabilities: [NetWrite, Module<smtp>]
help: add `NetWrite, Module<smtp>` to the `uses` of `process_request`
```

#### C0301 — effects-capability-conflict

```
error[C0301]: effects conflict with capabilities
  --> src/transform.spore:6:5
   |
 6 |     effects: pure
   |              ^^^^ `pure` is incompatible with `uses [StateWrite]`
   |
   = note: `pure` functions cannot have IO/State capabilities
help: either remove `pure` from effects or remove `StateWrite` from `uses`
```

### 5.4 K0xxx — Cost Violations

Cost errors arise when the compiler's abstract interpretation finds that a function exceeds its declared cost budget.

| Code | Name | Description |
|------|------|-------------|
| `K0101` | budget-exceeded | Concrete cost exceeds declared `cost ≤ K` |
| `K0102` | symbolic-budget-exceeded | Symbolic cost exceeds declared bound for some input sizes |
| `K0201` | unbounded-call | Calling an `unbounded` function without `with_cost_limit` |
| `K0202` | unbounded-recursion | Recursive function without structural termination proof |
| `K0301` | cost-declaration-missing | Function with cost-sensitive callees but no `cost ≤ K` declaration |

#### K0201 — unbounded-call

```
error[K0201]: calling `unbounded` function without cost limit
  --> src/math.spore:20:5
   |
20 |     fibonacci(n)
   |     ^^^^^^^^^^^^ `fibonacci` has `cost: unbounded`
   |
   = note: `compute` declares `cost ≤ 10000` but `fibonacci` has no static bound
help: wrap in `with_cost_limit(K) { fibonacci(n) }` and handle `CostExceeded`
```

#### K0102 — symbolic-budget-exceeded

```
error[K0102]: symbolic cost may exceed budget
  --> src/sort.spore:15:5
   |
15 |     fn sort_all(items: List<Int>) -> List<Int>
   |        ^^^^^^^^ cost is `N × log(N) × 3 + N` where N = len(items)
   |
   = note: declared `cost ≤ 5000`, but N is unbounded
   = note: for N = 1000, cost = 29,933 op > 5000 op
help: add a size constraint: `items: List<Int, max: 100>` or increase budget
```

### 5.5 H0xxx — Hole Diagnostics

Hole diagnostics are emitted as `note` severity — holes are valid states, not errors.

| Code | Name | Description |
|------|------|-------------|
| `H0101` | hole-report | Standard hole report with type, bindings, and candidates |
| `H0102` | hole-type-conflict | Explicit type annotation on hole conflicts with inferred type |
| `H0201` | partial-function | Function contains one or more unfilled holes |
| `H0202` | hole-cost-tight | Remaining cost budget for hole is very small (< 10% of total) |
| `H0301` | hole-duplicate-name | Two holes in the same module share a name |

#### H0102 — hole-type-conflict

```
warning[H0102]: hole type annotation conflicts with inference
  --> src/format.spore:51:20
   |
51 |     let body: Int = ?report_body : String
   |                     ^^^^^^^^^^^^^^^^^^^^^^^^ annotated as `String`, but context expects `Int`
   |
   = note: `body` is declared as `Int` on the left side of the binding
   = note: hole annotation `String` contradicts the binding type
help: change annotation to `: Int` or change binding type to `String`
```

#### H0202 — hole-cost-tight

```
note[H0202]: tight cost budget for hole `optimize_step`
  --> src/optimizer.spore:88:5
   |
88 |     ?optimize_step
   |     ^^^^^^^^^^^^^^ only 15 op remaining (3% of budget 500)
   |
   = note: prior code consumed 485 op
   = note: most candidate functions require > 50 op
help: consider increasing the function's cost budget or simplifying prior code
```

### 5.6 M0xxx — Module Errors

Module errors cover the structural and organizational rules of Spore's module system.

| Code | Name | Description |
|------|------|-------------|
| `M0101` | circular-dependency | Modules form a dependency cycle |
| `M0102` | self-import | Module imports itself |
| `M0201` | visibility-violation | Accessing a private or `pub(pkg)` symbol from outside its scope |
| `M0202` | re-export-visibility | Re-exporting a symbol with broader visibility than its original |
| `M0301` | import-not-found | Imported module or symbol does not exist |
| `M0302` | ambiguous-import | Two imports bring the same name into scope |
| `M0401` | snapshot-changed | Dependent function's signature hash changed; requires `--permit` |

#### M0301 — import-not-found

```
error[M0301]: import not found
  --> src/api.spore:2:5
   |
 2 | use billing.invoicer
   |     ^^^^^^^^^^^^^^^^ module `billing.invoicer` does not exist
   |
   = note: available modules in `billing`: billing.invoice, billing.payment, billing.report
help: did you mean `billing.invoice`?
```

#### M0401 — snapshot-changed

```
error[M0401]: signature hash changed — requires --permit
  --> src/order.spore:35:5
   |
35 |     let total = billing.calculate_total(items)
   |                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ signature hash mismatch
   |
   = note: expected sig@a3f7c2, found sig@b8d2e1
   = note: `calculate_total` added parameter `tax_rate: Float`
help: run `spore --permit` to accept the signature change, then update call sites
```

### 5.7 sporec --explain Output

`sporec --explain <CODE>` prints a detailed explanation of the error code, including common causes, examples, and fix strategies:

```
$ sporec --explain E0301

  E0301: type mismatch

  This error occurs when an expression has a type that is incompatible
  with the type expected by its context. Common contexts include:

  - Function arguments (actual type ≠ parameter type)
  - Return statements (expression type ≠ declared return type)
  - Variable bindings (right side type ≠ left side annotation)
  - Struct fields (value type ≠ field type)

  Example:
      fn greet(name: String) -> String { ... }
      greet(42)   -- error: expected String, found Int

  Common fixes:
  - Use a conversion function (e.g. `Int.to_string(42)`)
  - Change the declaration to accept the actual type
  - Add a type annotation to guide inference

  See also: E0302 (return type mismatch), E0401 (constraint not satisfied)
```

---

## 6. Fix Suggestions

### 6.1 When Suggestions Are Generated

Every diagnostic includes a `help:` line. Beyond textual suggestions, the compiler generates machine-applicable fixes when it can determine a concrete edit. Not all diagnostics have machine-applicable fixes — some only have informational suggestions.

### 6.2 Applicability Categories

| Category | Flag | Meaning |
|----------|------|---------|
| `safe` | `--fix` | Applying this fix preserves behavior and type safety. Example: adding a missing import. |
| `unsafe` | `--unsafe-fix` | Applying this fix may change behavior. Example: removing an unused variable that was intended for later use. |
| `informational` | *(not auto-applicable)* | Suggestion requires human judgment. Example: restructuring code to avoid a circular dependency. |

### 6.3 Fix Types

| Fix Type | Description | Example |
|----------|-------------|---------|
| **Replacement** | Replace a span with new text | `"fifty dollars"` → `Money.from_string("fifty dollars")` |
| **Insertion** | Insert text at a position | Adding `currency: Currency.USD` to a struct literal |
| **Deletion** | Remove a span | Removing a redundant match arm |
| **Multi-edit** | Multiple edits in one file | Adding a `with` clause and a `uses` clause |
| **Multi-file** | Edits across files | Adding `pub` in the defining module and updating the import in the consuming module |

### 6.4 Auto-Apply Workflow

```bash
# Preview safe fixes
$ sporec --fix --dry-run src/billing.spore
  Would apply 3 safe fixes (2 files affected)

# Apply safe fixes
$ sporec --fix src/billing.spore
  Applied 3 fixes. Re-checking...
  0 errors, 1 warning remaining.

# Apply all fixes (including unsafe)
$ sporec --fix --unsafe-fix src/billing.spore
  Applied 5 fixes (3 safe, 2 unsafe). Re-checking...
  0 errors, 0 warnings remaining.
```

### 6.5 Fix Suggestion Examples

#### Safe Fix: Missing Import

```json
{
  "description": "add missing import for `DateTime`",
  "applicability": "safe",
  "edits": [
    {
      "file": "src/scheduler.spore",
      "range": { "start": { "line": 2, "col": 1 }, "end": { "line": 2, "col": 1 } },
      "new_text": "use std.time.DateTime\n"
    }
  ]
}
```

#### Unsafe Fix: Remove Unused Variable

```json
{
  "description": "remove unused variable `temp`",
  "applicability": "unsafe",
  "edits": [
    {
      "file": "src/billing.spore",
      "range": { "start": { "line": 38, "col": 5 }, "end": { "line": 38, "col": 45 } },
      "new_text": ""
    }
  ]
}
```

#### Multi-File Fix: Add pub to Definition

```json
{
  "description": "make `helper_fn` public",
  "applicability": "unsafe",
  "edits": [
    {
      "file": "src/utils.spore",
      "range": { "start": { "line": 10, "col": 1 }, "end": { "line": 10, "col": 3 } },
      "new_text": "pub fn"
    }
  ]
}
```

---

## 7. Interaction with Spore Systems

### 7.1 Hole System Integration

Hole diagnostics (H0xxx) bridge the compiler output and the Hole system (see *hole-system-v0.2*). The relationship:

- **`sporec` compilation** emits `H0101` (hole-report) as `note` severity for each unfilled hole.
- **`sporec --query-hole <name>`** returns a full `HoleReport` — this is a *superset* of the H0101 diagnostic, including full candidate ranking, binding types, and cost budget. The HoleReport follows the same JSON schema as `--json` diagnostics but with additional fields.
- **Partial functions** (`H0201`) are compiled successfully — they produce diagnostics but not errors. The function is usable in simulation mode.

A HoleReport JSON object extends the diagnostic schema:

```json
{
  "severity": "note",
  "code": "H0101",
  "message": "hole `tax_logic` requires filling",
  "hole_report": {
    "name": "tax_logic",
    "expected_type": "Money",
    "bindings": {
      "income": "Money",
      "region": "Region"
    },
    "available_capabilities": ["TaxTable"],
    "cost_consumed": 100,
    "cost_budget_remaining": 400,
    "candidates": [
      {
        "function": "tax_table.lookup(region: Region, income: Money) -> Money",
        "cost": 50,
        "quality": "exact",
        "error_types": []
      }
    ],
    "pending_errors": []
  }
}
```

### 7.2 Cost System Integration

Cost diagnostics (K0xxx) are emitted when the abstract interpretation engine (see *cost-model-v0.1*) detects budget violations. The integration points:

- **K0101** (budget exceeded): emitted during pass [4] of the compilation flow (see cost-model §7.1). The diagnostic includes cost breakdown in the `inference_chain`.
- **K0102** (symbolic budget exceeded): emitted when symbolic expressions cannot be proven within bounds. The diagnostic includes the symbolic expression and a concrete counterexample.
- **K0201** (unbounded call): emitted during callee resolution when an `unbounded` function is called without `with_cost_limit`.
- **Cost context** in every diagnostic: the `context.cost_used` and `context.cost_budget` fields are populated for all diagnostics inside functions with `cost ≤ K` declarations, not just cost errors.

### 7.3 Capability System Integration

Capability diagnostics (C0xxx) enforce the capability algebra at three levels:

1. **Function level** (C0101): a function's body uses operations beyond its `uses` declaration.
2. **Module level** (C0102): a function's `uses` exceeds the module's capability ceiling.
3. **Platform level** (C0201): a package requests capabilities the Platform does not grant.

The `context.capabilities` field in every diagnostic lists the capabilities in scope at the error site, enabling Agents to reason about what operations are available.

### 7.4 Snapshot System Integration

Snapshot diagnostics (M0401) connect the compiler output to the snapshot/hash system (see *signature-syntax-v0.2*, §Snapshot Hash):

- When a dependent function's signature hash changes, `sporec` emits `M0401` with both the expected and actual hashes.
- The `suggested_fix` for M0401 is always `informational` — the developer must explicitly `spore --permit` to accept the change.
- The `related` field points to the changed signature's declaration.

### 7.5 Module System Integration

Module diagnostics (M0xxx) enforce the structural rules from *module-system-v0.1*:

- **Circular dependencies** (M0101): detected during dependency resolution. The `inference_chain` shows the full cycle path.
- **Visibility violations** (M0201): the `candidates` field may list public alternatives with similar names.
- **Import resolution** (M0301): the `suggested_fix` uses fuzzy matching to suggest the closest valid import.

---

## 8. Agent Consumption Patterns

### 8.1 Reading --json Output

Agents should consume `sporec --json` output. The structured format enables programmatic reasoning without parsing human-readable text. A typical Agent reads:

1. `diagnostics[]` — iterate over all diagnostics
2. `severity` — filter by error (must fix) vs. warning (should fix) vs. note (informational)
3. `code` — categorize by prefix (E/W/C/K/H/M) for specialized handling
4. `context.hole` — if non-null, this diagnostic is hole-related; use `--query-hole` for full report
5. `suggested_fix.edits` — if `applicability == "safe"`, apply directly; if `"unsafe"`, apply with caution

### 8.2 Choosing Between Modes

| Scenario | Recommended Mode |
|----------|-----------------|
| Agent filling holes | `--json` (parse diagnostics, apply fixes) |
| Agent debugging complex type error | `--json` (use `inference_chain` and `candidates`) |
| Human scanning build output | Default (concise, color-coded) |
| Human debugging inference failure | `--verbose` (see inference chain in terminal) |
| CI/CD pipeline | `--json --stream` (machine-readable, early feedback) |
| IDE / LSP client | `--json` (map to LSP Diagnostics) |

### 8.3 Integration with HoleReport

The `--query-hole` output follows the same schema as `--json` diagnostics, extended with the `hole_report` field (§7.1). This means an Agent can use a single JSON parser for both compilation diagnostics and hole queries.

```bash
# Compile, get diagnostics
$ sporec --json src/tax.spore > diagnostics.json

# For each hole diagnostic, get full report
$ sporec --query-hole tax_logic --json > hole_report.json
```

Both files share the same `Diagnostic` schema, enabling unified processing.

### 8.4 Example Agent Workflow

A typical Agent workflow for filling a hole:

```
1. sporec --json src/tax.spore
   → Parse diagnostics
   → Find H0101 for hole `tax_logic`

2. sporec --query-hole tax_logic --json
   → Read full HoleReport
   → expected_type: Money
   → bindings: {income: Money, region: Region}
   → candidates: [tax_table.lookup, tax_table.calculate]
   → cost_budget_remaining: 400

3. Agent decides: tax_table.lookup costs 50 op, within budget, exact match
   → Generate fill: `tax_table.lookup(region, income)`

4. Agent applies edit to src/tax.spore line 12

5. sporec --json src/tax.spore
   → Re-check: 0 errors, 0 warnings, 0 holes
   → Done.
```

### 8.5 Agent Error Recovery

When an Agent's fix introduces new errors, the diagnostic system supports iterative repair:

```
1. Agent applies fix for E0301
2. sporec --json → new E0303 (error type mismatch introduced)
3. Agent reads inference_chain for E0303 → understands the new error
4. Agent applies second fix
5. sporec --json → clean
```

The `inference_chain` and `candidates` fields are specifically designed to give Agents enough context to reason about errors without needing to re-read source files.

---

## 9. Design Rationale

### 9.1 Why Rust Style Over Elm Style

Elm's compiler messages are paragraphs of natural language — friendly for beginners but verbose for experienced developers and expensive for Agents to parse. Rust's style is:

- **Concise**: a single error fits in 5–8 lines, not 15–20.
- **Structured**: the layout (gutter, pointer, underline) is visually scannable.
- **Machine-friendly**: even the default text output has consistent structure that Agents can regex-parse in a pinch.
- **Scalable**: 50 Elm-style errors flood the terminal; 50 Rust-style errors are manageable.

Spore adopts Rust's layout while keeping Elm's philosophy of *helpfulness*: every diagnostic always includes a `help:` line. The result is concise *and* actionable.

### 9.2 Why Always Include Help

A diagnostic without a suggestion is a dead end. The developer sees what's wrong but not how to fix it. By mandating that every diagnostic includes `help:`, we ensure:

- Beginners always have a next step.
- Agents can extract a fix suggestion without additional reasoning.
- The compiler team is forced to think about actionability when defining new error codes.

Some `help:` lines are concrete (`try Money.from_string(...)`) and some are strategic (`extract shared types into a new module`). Both are valuable.

### 9.3 Why Category-Based Error Codes

Error codes like `E0301` are more useful than raw names because:

- **Filterable**: `sporec --json | jq '.diagnostics[] | select(.code | startswith("K"))'` gives all cost errors.
- **Discoverable**: `sporec --explain E0301` opens documentation. Users remember codes they see frequently.
- **Stable**: codes don't change when messages are reworded. CI/CD pipelines can allowlist specific codes.
- **Cross-referencing**: documentation, forums, and issue trackers can reference `E0301` unambiguously.

The prefix convention (E/W/C/K/H/M) maps directly to Spore's major subsystems, making it immediately clear which part of the language a diagnostic belongs to.

### 9.4 Why LSP Compatibility

LSP (Language Server Protocol) is the industry standard for IDE integration. By designing `--json` output to map directly to LSP `Diagnostic` objects:

- **Zero-cost IDE integration**: the LSP server wraps `sporec --json` output with minimal transformation.
- **Extension-friendly**: Spore-specific data lives in the `data.spore` namespace, invisible to generic LSP clients but available to Spore-aware editors.
- **Future-proof**: as LSP evolves, the mapping stays clean because we follow its conventions.

### 9.5 Why JSON Is the Superset

Making `--json` the superset of all information eliminates a common problem: different output modes showing contradictory information. With a single underlying data model:

- Default mode renders a *projection* of the JSON.
- Verbose mode renders a *larger projection*.
- JSON mode renders *everything*.

This ensures consistency: if the default output shows `expected Money, found String`, the JSON will contain the same message plus the inference chain that led to that conclusion. No information is generated exclusively for one mode.

---

## 10. Open Questions

1. **Diagnostic deduplication**: When the same root cause produces multiple downstream errors (e.g., a wrong type propagates through 5 call sites), how aggressively should the compiler deduplicate? Current proposal: show the root error in full, and collapse downstream errors into a single `= note: N additional errors caused by this`.

2. **Diagnostic ordering**: Should errors be ordered by file position, severity, or category? Current proposal: primary sort by file, secondary by line number, with errors before warnings before notes within the same line.

3. **Streaming granularity for --json --stream**: Should streaming emit per-file, per-function, or per-diagnostic? Current proposal: per-diagnostic (NDJSON), which gives the finest granularity and earliest feedback.

4. **Internationalization**: Should diagnostic messages support translation? Current position: English only for v0.1. Error codes are language-independent, so `sporec --explain` could be localized independently in a future version.

5. **Diagnostic suppression**: Should there be a mechanism to suppress specific codes (e.g., `#[allow(W0301)]`)? Current proposal: yes, using inline annotations and project-level configuration in `spore.toml`, but the detailed syntax is deferred to a future design document.

6. **Performance budget visualization**: For `--verbose` cost errors, should the compiler emit an ASCII bar chart of cost distribution? This could help developers visually identify the expensive call. Deferred pending user feedback.
