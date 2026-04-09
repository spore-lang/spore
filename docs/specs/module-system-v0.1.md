# Spore Module System — Design Document v0.1

> **Status**: Draft
> **Scope**: `sporec` compiler, `spore` Codebase Manager, Agent workflow
> **Depends on**: Signature system, Cost system, Capability system, Snapshot system, Hole system, Error system

---

## 0. Guiding Principles

| Principle | Implication |
|---|---|
| **Signatures are gravity centers** | Modules organize functions by signature. A module's public API is defined by the signatures it exports — implementations are behind the curtain. |
| **Capabilities, not ambient authority** | A module cannot perform IO unless a Platform grants it. Imported packages are pure by default. |
| **Agent-friendly** | Module boundaries, visibility rules, and dependency graphs are machine-readable. An Agent can enumerate a module's public API, understand its capability requirements, and navigate the dependency DAG without human guidance. |
| **Simplicity over expressiveness** | No functors, no parameterized modules, no recursive modules. Generics and capabilities cover the same ground with less conceptual overhead. |
| **Dual-hash content addressing** | Functions carry two hashes: a *signature hash* (`sig`) for API compatibility and an *implementation AST hash* (`impl`) for exact content addressing. `sig` changes break dependents; `impl` changes do not. Partial functions have `impl = None`. |

---

## 1. Overview & Philosophy

Spore's module system is the organizational backbone of the language. It answers three questions:

1. **Where does code live?** — One `.spore` file = one module. No ambiguity.
2. **Who can see what?** — Default private, explicit `pub` / `pub(pkg)` for visibility.
3. **What can code do?** — Capabilities flow from Platforms through packages to functions. Modules have no separate capability carrier or ceiling; only function signatures and package/Platform boundaries are checked.

### 1.1 Design Coordinates

The module system sits at the intersection of several Spore subsystems:

```
                    ┌─────────────┐
                    │  Platform   │  provides capabilities
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │   Package   │  spore.toml, dependency declarations
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │   Module    │  one .spore file, file-derived name
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
         ┌────────┐  ┌────────┐  ┌────────┐
         │Function│  │  Type  │  │ Alias  │
         └────────┘  └────────┘  └────────┘
         sig + impl   named       re-export
```

### 1.2 Dual Hash Addressing

Every function in Spore has two addresses for everyday use and two hashes for machine use:

- **Human address**: `billing.invoice.generate_invoice` — readable, navigable.
- **Machine address**: `billing.invoice.generate_invoice` with `sig=a3f7c2, impl=d8e1b4` recorded in `.spore-lock`.

The two hashes serve distinct purposes:

1. **Signature hash (`sig`)**: Covers parameter names, parameter types, return type, error types, effects, cost bound, capabilities, and generic constraints. Used for **API compatibility checking**. Does NOT change when the implementation changes.

2. **Implementation AST hash (`impl`)**: Covers the compiled AST of a successfully compiled function. Used for **content addressing** — it uniquely identifies the exact code. For **partial functions** (functions containing holes), `impl = None` because they are incomplete and have no finalized implementation to hash.

This means:

- Filling a hole: `impl` goes from `None` to a concrete hash; `sig` stays the same.
- Refactoring a function body: `impl` changes; `sig` stays the same. Dependents do not recompile.
- Adding a parameter **does** change `sig`. Dependents must `spore --permit`.
- Compatibility check: only looks at `sig` — if `sig` is unchanged, the interface is compatible, and dependents don't recompile.
- Content locking: looks at `impl` — exact code is pinned in `.spore-lock`.
- Breaking change detection: `sig` changed → needs `spore --permit`.

This is the content-addressed foundation of Spore's dependency model, inspired by Unison's hash-everything approach but applied as a dual-hash scheme: `sig` for stability (preserving file-based tooling) and `impl` for deterministic content addressing (pinning exact code). An earlier design considered tracking only a "last implementation" hash, but this was rejected as too fuzzy and non-deterministic — the AST hash is computed directly from the compiled representation and is unambiguous.

---

## 2. Module Basics

### 2.1 File = Module

Every `.spore` file is exactly one module. The module name is derived from its path relative to the package `src/` root:

| File Path | Module Name |
|---|---|
| `src/billing/invoice.spore` | `billing.invoice` |
| `src/auth/token.spore` | `auth.token` |
| `src/utils.spore` | `utils` |
| `src/main.spore` | `main` |

There is no separate module declaration syntax — the filesystem **is** the declaration. The compiler verifies that the directory structure forms a valid module tree at build time.

### 2.2 No In-File Module Declaration

Modules do **not** begin with a `module ...` declaration. The file path already determines the module name, so the source file starts directly with imports and declarations:

```spore
-- src/billing/invoice.spore
import billing.types as types
import billing.tax as tax

pub fn generate_invoice(order: Order) -> Invoice ! TaxError | ValidationError
    uses [PaymentGateway, AuditLog]
    cost [3000, 0, 0, 0]
{
    let tax_result = tax.calculate(order.items, order.region)
    let line_items = build_line_items(order.items, tax_result)
    finalize(order.customer, line_items)
}

fn build_line_items(items: Vec[Item], tax_result: TaxResult) -> Vec[LineItem]
    cost [500, 0, 0, 0]
{
    items |> map(fn(item) -> to_line_item(item, tax_result))
}

fn finalize(customer: Customer, items: Vec[LineItem]) -> Invoice ! ValidationError
    uses [AuditLog]
    cost [1000, 0, 0, 0]
{
    let invoice = Invoice.new(customer, items)
    audit_log.record("invoice_created", invoice.id)
    invoice
}
```

Rules:

- There is **no** optional module header.
- A file's module name is always derived from its relative path under `src/`.
- Source files carry no module-level capability metadata; capability checking stays on functions and package/Platform configuration.

### 2.3 Directory Structure Conventions

A typical Spore package:

```
my-billing-lib/
├── spore.toml              -- package manifest
├── src/
│   ├── billing/
│   │   ├── invoice.spore   -- module: billing.invoice
│   │   ├── tax.spore       -- module: billing.tax
│   │   ├── types.spore     -- module: billing.types
│   │   └── shortcuts.spore -- module: billing.shortcuts (pub aliases)
│   └── utils.spore         -- module: utils
└── test/
    ├── billing/
    │   ├── invoice_test.spore
    │   └── tax_test.spore
    └── utils_test.spore
```

Conventions (not enforced, but recommended):

- `src/` contains all source modules.
- `test/` contains test modules. Tests can import `pub` and `pub(pkg)` items from `src/`.
- One module per concern. Prefer many small modules over few large ones.
- A `types.spore` module in a directory holds shared type definitions for that directory's modules.
- A `shortcuts.spore` module can hold `pub alias` definitions for commonly-used items.

### 2.4 Empty Modules

An empty module is valid. It compiles successfully and exports nothing:

```spore
-- src/billing/future.spore
-- This module is a placeholder for future billing features.
```

```
$ sporec check src/billing/future.spore

[ok] billing.future
  exports: (none)
  capabilities: []
```

### 2.5 Module Name Rules

Module names follow these constraints:

- Segments are lowercase `snake_case`: `billing_v2.invoice_generator`
- Segments are separated by `.` (derived from `/` in the file path)
- No leading or trailing dots
- No consecutive dots
- Reserved names: `main`, `platform`, `test` have special semantics (see §7, §10)

---

## 3. Visibility

### 3.1 Three Visibility Levels

| Level | Keyword | Meaning |
|---|---|---|
| **Private** | *(default)* | Visible only within the defining module |
| **Package-internal** | `pub(pkg)` | Visible to any module within the same package |
| **Public** | `pub` | Visible to any importer, including external packages |

### 3.2 Visibility on Functions

```spore
-- src/billing/invoice.spore

-- Public: any importer can call this
pub fn generate_invoice(order: Order) -> Invoice ! TaxError {
    let validated = validate(order)
    compute_totals(validated)
}

-- Package-internal: other modules in this package can call this,
-- but external packages cannot
pub(pkg) fn validate(order: Order) -> ValidatedOrder ! ValidationError {
    ...
}

-- Private: only this module can call this
fn compute_totals(order: ValidatedOrder) -> Invoice {
    ...
}
```

### 3.3 Visibility on Types

```spore
-- src/billing/types.spore

-- Public type: external packages can use this
pub type Invoice {
    id: InvoiceId,
    customer: Customer,
    line_items: Vec<LineItem>,
    total: Money,
}

-- Package-internal type: used across modules within this package
pub(pkg) type ValidatedOrder {
    original: Order,
    validation_timestamp: Timestamp,
}

-- Private type: only used in this module
type InternalCache {
    entries: Map[Str, CacheEntry],
}
```

### 3.4 Visibility on Aliases

```spore
-- src/billing/shortcuts.spore

-- Public alias: importers of this module get access to this alias
pub alias GenInv = billing.invoice.generate_invoice
pub alias Inv = billing.types.Invoice

-- Package-internal alias
pub(pkg) alias Validate = billing.invoice.validate

-- Private alias (for local convenience)
alias Compute = billing.invoice.compute_totals
```

### 3.5 Visibility Error Messages

Attempting to access a private item:

```
$ sporec check src/api/handler.spore

[error] visibility violation at src/api/handler.spore:15
  billing.invoice.compute_totals is private
  ─── it is only visible within module billing.invoice

  help: if this function should be accessible to other modules,
        add `pub` or `pub(pkg)` to its definition in src/billing/invoice.spore
```

Attempting to access a `pub(pkg)` item from an external package:

```
$ sporec check

[error] visibility violation at src/handler.spore:8
  billing.invoice.validate is pub(pkg)
  ─── it is only visible within the 'my-billing-lib' package
  ─── your module 'handler' is in the 'my-api' package

  help: use the public API instead — billing.invoice.generate_invoice
        handles validation internally
```

### 3.6 Visibility and the Hole System

Holes inherit the visibility context of their enclosing function. A HoleReport includes only candidates that are **visible** from the hole's location:

```json
{
  "hole": { "name": "payment_logic" },
  "candidates": [
    {
      "function": "billing.invoice.generate_invoice",
      "visibility": "pub",
      "accessible": true
    },
    {
      "function": "billing.invoice.validate",
      "visibility": "pub(pkg)",
      "accessible": true,
      "note": "same package"
    }
  ],
  "excluded": [
    {
      "function": "billing.invoice.compute_totals",
      "visibility": "private",
      "reason": "private to billing.invoice"
    }
  ]
}
```

---

## 4. Imports & Aliases

### 4.1 Import Syntax

Spore provides exactly two import mechanisms — `import` and `alias` — with no overlap.

#### Module Import

```spore
import billing.invoice
```

After this, all `pub` items of `billing.invoice` are accessible via qualified name:

```spore
let inv = billing.invoice.generate_invoice(order)
```

#### Module Import with Rename

```spore
import billing.invoice as inv
```

Now the module is accessed as `inv`:

```spore
let invoice = inv.generate_invoice(order)
```

#### Item Alias

```spore
alias gen = billing.invoice.generate_invoice
alias Inv = billing.types.Invoice
```

Now specific items are accessible by their alias name:

```spore
let invoice: Inv = gen(order)
```

### 4.2 Import Rules

| Rule | Description |
|---|---|
| `import` operates on **modules** only | You cannot `import` a function or type directly |
| `as` works only with `import` | `import X as Y` is valid; `alias X as Y` is not |
| `alias` operates on **items** only | You cannot `alias` a module; use `import ... as` instead |
| No wildcard imports | There is no `import billing.*` or `import billing.invoice exposing (..)` |
| No implicit nested imports | `import billing` does **not** import `billing.invoice` or `billing.tax` |
| No re-export via import | Importing a module does not make its items part of your exports |

### 4.3 Pub Aliases for Convenience Modules

A common pattern: a module defines `pub alias` items to provide a shortcut interface:

```spore
-- src/billing/shortcuts.spore

import billing.invoice
import billing.tax
import billing.types

pub alias generate = billing.invoice.generate_invoice
pub alias calculate_tax = billing.tax.calculate
pub alias Invoice = billing.types.Invoice
pub alias TaxResult = billing.types.TaxResult
```

Consumers import the shortcuts module:

```spore
-- src/api/handler.spore
import billing.shortcuts as bill

fn handle_order(order: Order) -> Response ! BillingError {
    let tax = bill.calculate_tax(order.items, order.region)
    let invoice: bill.Invoice = bill.generate(order)
    Response.ok(invoice)
}
```

This is the only re-export mechanism. There is no implicit forwarding.

### 4.4 Invalid Import Examples

```spore
-- ERROR: cannot import a function directly
import billing.invoice.generate_invoice
```

```
[error] invalid import at line 1
  billing.invoice.generate_invoice is a function, not a module
  ─── `import` works on modules only

  help: use `alias gen = billing.invoice.generate_invoice` instead
```

```spore
-- ERROR: cannot use `as` with alias
alias billing.invoice.generate_invoice as gen
```

```
[error] syntax error at line 1
  `as` is only valid with `import` (module-level renaming)

  help: use `alias gen = billing.invoice.generate_invoice`
```

```spore
-- ERROR: cannot alias a module
alias bill = billing.invoice
```

```
[error] invalid alias at line 1
  billing.invoice is a module, not an item
  ─── `alias` works on specific items (functions, types) only

  help: use `import billing.invoice as bill` instead
```

```spore
-- ERROR: wildcard imports are not supported
import billing.*
```

```
[error] syntax error at line 1
  wildcard imports are not supported in Spore
  ─── import modules explicitly: `import billing.invoice`

  rationale: wildcard imports make dependency tracking ambiguous
             and hinder Agent-based code analysis
```

### 4.5 Import Grammar (Formal)

```
import_decl  ::= 'import' module_path ('as' IDENT)?
alias_decl   ::= 'alias' IDENT '=' qualified_item
module_path  ::= IDENT ('.' IDENT)*
qualified_item ::= module_path '.' IDENT
```

### 4.6 Import Ordering Convention

The compiler does not enforce import ordering, but `sporec fmt` sorts imports into groups:

```spore
-- Platform / standard library imports
import std.collections
import std.text

-- External package imports
import http.client as http
import json.parser as json

-- Internal package imports
import billing.invoice
import billing.types

-- Aliases
alias Invoice = billing.types.Invoice
alias gen = billing.invoice.generate_invoice
```

---

## 5. Content-Addressed Functions

### 5.1 Dual Hash in Module Context

Every function's identity includes its module path and two hashes — `sig` (signature) and `impl` (implementation AST):

```
billing.invoice.generate_invoice  sig=a3f7c2  impl=d8e1b4
```

The `sig` hash `a3f7c2` (truncated for display; full hash is 32 hex characters) is computed from the **canonical signature**:

```
fn generate_invoice(order: Order) -> Invoice ! TaxError | ValidationError
    cost [3000, 0, 0, 0]
    uses [PaymentGateway, AuditLog]
```

### 5.2 What Changes Each Hash

| Change | `sig` changes? | `impl` changes? |
|---|---|---|
| Function name: `generate_invoice` → `create_invoice` | **Yes** | **Yes** |
| Parameter name: `order` → `purchase_order` | **Yes** | **Yes** |
| Parameter type: `Order` → `OrderRequest` | **Yes** | **Yes** |
| Parameter order: `(a, b)` → `(b, a)` | **Yes** | **Yes** |
| Return type: `Invoice` → `InvoiceResult` | **Yes** | **Yes** |
| Error type set: Add `[ValidationError]` | **Yes** | **Yes** |
| Effects: `pure` → `deterministic` | **Yes** | **Yes** |
| Cost bound: `≤ 3000` → `≤ 5000` | **Yes** | **Yes** |
| Capabilities: Add `AuditLog` | **Yes** | **Yes** |
| Generic constraints: `T: Eq` → `T: Eq + Hash` | **Yes** | **Yes** |
| Function body: Refactor internals | **No** | **Yes** |
| Hole filling: Replace `?logic` with implementation | **No** | `None` → concrete hash |
| Comments: Add/remove/edit | **No** | **No** |
| Formatting: Reformat code | **No** | **No** |

Key insight: signature changes always change both hashes (the implementation necessarily differs too). But body-only changes update `impl` while leaving `sig` untouched — this is the decoupling that prevents unnecessary downstream recompilation.

### 5.3 `.spore-lock` and Module Dependencies

The `.spore-lock` file records both hashes for all functions that a module depends on:

```toml
# .spore-lock (auto-generated, do not edit)

[deps.api.handler.generate_invoice]
sig  = "a3f7c2"    # interface contract
impl = "d8e1b4"    # exact implementation (None if partial)

[deps.api.handler.calculate]
sig  = "d91e4b"
impl = "b2c4a7"

[deps.api.handler.Invoice]
sig  = "8c3f01"
impl = "e9f3d2"

[deps.api.handler.TaxResult]
sig  = "f72a9e"
impl = "17a8c6"

[deps.billing.invoice.calculate]
sig  = "d91e4b"
impl = "b2c4a7"

[deps.billing.invoice.Order]
sig  = "1b84c3"
impl = "4d7e9a"

[deps.billing.invoice.Invoice]
sig  = "8c3f01"
impl = "e9f3d2"
```

Compatibility checking uses only the `sig` field: if `sig` is unchanged, dependents do not need to recompile. The `impl` field pins the exact code version for reproducible builds and content locking.

### 5.4 Change Detection and `spore --permit`

When a signature changes, the `sig` hash changes. All downstream modules that depend on the old `sig` are flagged:

```
$ sporec check

[warning] signature changed: billing.tax.calculate
  old sig: d91e4b
  new sig: 7a2f33
  change: added error type [RegionNotSupported]

  affected modules:
    billing.invoice (depends on billing.tax.calculate sig=d91e4b)
    api.handler     (depends on billing.tax.calculate sig=d91e4b)

  run `spore --permit billing.tax.calculate` to accept this change
  and update all downstream lock entries
```

Accepting the change:

```
$ spore --permit billing.tax.calculate

Accepted signature change for billing.tax.calculate
  new sig: 7a2f33
  updated .spore-lock entries:
    billing.invoice: billing.tax.calculate sig d91e4b → 7a2f33
    api.handler:     billing.tax.calculate sig d91e4b → 7a2f33

  ⚠ billing.invoice may need code changes to handle [RegionNotSupported]
  ⚠ api.handler may need code changes to handle [RegionNotSupported]
```

Note: `spore --permit` updates the `sig` field in `.spore-lock`. The `impl` field is updated automatically whenever the implementation is recompiled — no permit is needed for `impl`-only changes.

### 5.5 Batch Permit

When multiple signatures change (e.g., during a refactor):

```
$ spore --permit --all

Accepted 3 signature changes:
  billing.tax.calculate              sig d91e4b → 7a2f33
  billing.types.TaxResult            sig f72a9e → a1b2c3
  billing.invoice.generate_invoice   sig a3f7c2 → e4d5f6

Updated .spore-lock (12 sig entries changed; impl entries updated automatically)
```

### 5.6 Relationship to Snapshot System

The `.spore-lock` file is an extension of the existing snapshot system, now tracking both hashes:

- **Per-function snapshots**: Each function has a `sig` hash (from its canonical signature) and an `impl` hash (from its compiled AST). Partial functions have `impl = None`.
- **Per-module snapshots**: A module's "interface hash" is the hash of all its `pub` and `pub(pkg)` `sig` hashes, sorted deterministically. (Implementation hashes do not affect the interface hash — only the API contract matters at the module boundary.)
- **Per-package snapshots**: A package's "API hash" is the hash of all module interface hashes for modules containing `pub` items.

```
$ spore snapshot --show billing.invoice

Module: billing.invoice
  interface hash: 4c8e2a (from 1 pub function, 0 pub(pkg) functions)

  pub generate_invoice  sig=a3f7c2  impl=d8e1b4
    (Order) -> Invoice ! TaxError | ValidationError
```

For a partial module:

```
$ spore snapshot --show billing.invoice

Module: billing.invoice (partial)
  interface hash: 4c8e2a (from 1 pub function, 0 pub(pkg) functions)

  pub generate_invoice  sig=a3f7c2  impl=None (partial — contains holes)
    (Order) -> Invoice ! TaxError | ValidationError
```

When a hole is filled, `sig` and the interface hash remain unchanged. Only `impl` transitions from `None` to a concrete hash. Dependents are unaffected.

---

## 6. Module-Level Capabilities

Spore does **not** define module-level capability carriers or module-level capability ceilings.
Capability checking is intentionally scoped to:

- function signatures via `uses [...]`
- package / application ceilings in `spore.toml`
- Platform grants

Importing a module never grants ambient authority, and modules do not have their own hidden capability metadata apart from the functions they contain.

### 6.1 Explicitly Out of Scope

- header-based `module ... uses ...` syntax
- inferred or compiler-written module capability headers
- diagnostics about module-level capability ceilings

If the project ever revisits module-level capabilities, that would be a new design rather than latent v0.1 syntax.

### 6.4 Capability Propagation via Import

Importing a module does **not** grant its capabilities to the importer. Capabilities are per-function, declared in `uses [...]`:

```spore
-- src/api/handler.spore
import billing.invoice as invoice

pub fn handle_create_invoice(req: Request) -> Response ! ApiError
    uses [PaymentGateway, AuditLog]
{
    let order = parse_order(req.body)
    let created = invoice.generate_invoice(order)
    Response.ok(created)
}
```

The rule: if you call a function that requires capability `C`, your function must also declare `uses [C]` (or a superset). Capabilities propagate **upward** through the call graph.

---

## 7. Platform 系统

> **详细规范请参阅 [Platform 系统规范](platform-system-v0.1.md)。**
>
> 本节仅提供模块系统与 Platform 系统交互的概要。

Platform 提供能力的具体实现（effect handler）。每个应用通过 `spore.toml` 指定 Platform：

- Platform 实现 `uses [...]` 中声明的能力集（如 `Console`、`FileRead`）
- 编译器验证应用所需能力 ⊆ Platform 提供能力
- 多 Platform 支持通过优先级路由

---

## 8. Dependency Rules

### 8.1 No Circular Dependencies

Module A cannot import module B if B (directly or transitively) imports A. This is enforced at compile time.

```spore
-- src/billing/invoice.spore
import billing.payment       -- OK

-- src/billing/payment.spore
import billing.invoice       -- ERROR: circular dependency
```

```
[error] circular dependency detected

  billing.invoice
    → imports billing.payment
      → imports billing.invoice  ← cycle!

  cycle path: billing.invoice → billing.payment → billing.invoice

  help: extract shared types or functions into a third module
        e.g., create billing.types with items used by both modules
```

### 8.2 Transitive Cycle Detection

The compiler detects cycles of any length:

```
[error] circular dependency detected

  billing.invoice
    → imports billing.payment
      → imports billing.reconciliation
        → imports billing.invoice  ← cycle!

  cycle path: billing.invoice → billing.payment → billing.reconciliation → billing.invoice

  help: extract shared types or functions into a new module
        that all three can depend on without forming a cycle
```

### 8.3 Resolution Patterns

The standard resolution: extract shared definitions into a separate module.

**Before** (cycle):

```
billing.invoice ←──→ billing.payment
    (both need Invoice type and PaymentResult type)
```

**After** (DAG):

```
billing.types   (defines Invoice, PaymentResult)
     ↑      ↑
     │      │
billing.invoice  billing.payment
     (imports types)   (imports types)
```

```spore
-- src/billing/types.spore
pub type Invoice { ... }
pub type PaymentResult { ... }
pub type Order { ... }

-- src/billing/invoice.spore
import billing.types
-- no longer needs billing.payment

-- src/billing/payment.spore
import billing.types
-- no longer needs billing.invoice
```

### 8.4 Topological Build Order

The compiler processes modules in topological order of their dependency graph:

```
$ sporec build --show-order

Build order (topological):
  1. billing.types       (0 dependencies)
  2. billing.tax         (1 dependency: billing.types)
  3. billing.invoice     (2 dependencies: billing.types, billing.tax)
  4. billing.payment     (1 dependency: billing.types)
  5. billing.shortcuts   (3 dependencies: billing.types, billing.invoice, billing.tax)
  6. api.handler         (2 dependencies: billing.invoice, billing.shortcuts)
  7. main                (1 dependency: api.handler)

Parallelizable:
  Level 0: [billing.types]
  Level 1: [billing.tax, billing.payment]
  Level 2: [billing.invoice]
  Level 3: [billing.shortcuts]
  Level 4: [api.handler]
  Level 5: [main]
```

Modules at the same topological level can be compiled in parallel.

### 8.5 Diamond Dependencies

Diamond dependencies are allowed (they are not cycles):

```
       billing.types
        ↑        ↑
        │        │
  billing.invoice  billing.tax
        ↑        ↑
        │        │
       api.handler
```

Both `billing.invoice` and `billing.tax` depend on `billing.types`. `api.handler` depends on both. This is fine — `billing.types` is compiled once, and both dependents see the same module.

The `.spore-lock` ensures that the same `sig` hash for `billing.types.Invoice` is used everywhere, and `impl` pins the exact version:

```toml
[deps.billing.invoice.Invoice]
sig  = "8c3f01"
impl = "e9f3d2"

[deps.billing.tax.Invoice]
sig  = "8c3f01"
impl = "e9f3d2"

[deps.api.handler.generate_invoice]
sig  = "a3f7c2"
impl = "d8e1b4"

[deps.api.handler.calculate]
sig  = "d91e4b"
impl = "b2c4a7"
# Both transitively depend on billing.types.Invoice sig=8c3f01
```

---

## 9. Interaction with Other Spore Systems

### 9.1 Hole System

Holes respect module boundaries:

- A hole in module A can only be filled with code that respects the enclosing function's `uses [...]` requirements and any package/platform ceilings in force.
- A hole's candidates (functions listed in the HoleReport) are filtered by visibility — only items visible from the hole's location are suggested.
- Partial modules (modules containing holes) are valid compilation units. They export their `pub` items normally, but those items are marked `partial` and have `impl = None` in `.spore-lock` (no implementation hash is assigned until all holes are filled).

```
$ sporec --holes

Holes by module:

  billing.invoice (partial)
    ?invoice_logic at line 12
      type: Invoice ! TaxError
      capabilities: [PaymentGateway, AuditLog]
      budget remaining: 2400 ops

  api.handler (partial — depends on partial billing.invoice)
    (no direct holes, but transitively partial)
```

A module that calls a partial function becomes **transitively partial**. The compiler tracks this:

```spore
-- src/api/handler.spore
import billing.invoice

pub fn handle(req: Request) -> Response ! ApiError
    uses [PaymentGateway, AuditLog]
{
    -- generate_invoice is partial (contains holes)
    -- so handle is transitively partial
    let inv = billing.invoice.generate_invoice(parse_order(req))
    Response.ok(inv)
}
```

```
$ sporec check src/api/handler.spore

[partial] api.handler.handle : (Request) -> Response ! ApiError
  depends on partial: billing.invoice.generate_invoice
  ─── this function will become complete when billing.invoice.generate_invoice
      has all its holes filled
```

### 9.2 Cost Model

#### Module-Level Cost Budgets

Spore does not enforce module-level cost budgets. Cost is a per-function property declared via `cost [compute, alloc, io, parallel]`. However, the `spore` tool can report aggregate cost information per module:

```
$ spore cost-report billing.invoice

Module: billing.invoice

  Function costs:
    generate_invoice    cost [3000, 0, 0, 0]  (measured: [2800, 40, 2, 0])
    void_invoice        cost [500, 0, 0, 0]   (measured: [320, 12, 1, 0])

  Module aggregate:
    max single-call cost: 3000 ops (generate_invoice)
    total declared budget: 3500 ops
```

#### Cost and Imports

When a function calls an imported function, the callee's declared cost bound is used for cost estimation:

```spore
-- billing.tax.calculate has: cost [800, 0, 0, 0]
-- billing.invoice.generate_invoice calls calculate:
--   cost contribution = 800 (from calculate's declared bound)
```

If the callee's actual cost is lower than its declared bound, the caller's actual cost will be lower than its estimate. The declared bound provides a safe upper limit.

### 9.3 Snapshot System

Snapshots operate at three granularities in the module context, using both hashes:

| Level | What Is Hashed | When It Changes |
|---|---|---|
| **Function `sig`** | Canonical signature (params, returns, errors, effects, cost, capabilities, constraints) | Any signature component changes |
| **Function `impl`** | Compiled AST of the function body | Body changes, hole filling (`None` → concrete), or signature changes |
| **Module interface** | Sorted hash of all `pub` + `pub(pkg)` function `sig` hashes | Any exported function's signature changes |
| **Package API** | Sorted hash of all module interface hashes (for modules with `pub` items) | Any module's exported interface changes |

Note: Module interface and Package API hashes are derived from `sig` hashes only — `impl` changes do not propagate to these levels. This is what makes body refactoring and hole filling invisible to dependents.

```
$ spore snapshot

Package: billing-lib v1.2.0
  API hash: f3a9c1

  Module interfaces:
    billing.invoice   interface: 4c8e2a  (1 pub fn)
    billing.tax       interface: b7d3e5  (2 pub fns)
    billing.types     interface: 9a1f4c  (4 pub types)
    billing.shortcuts interface: 2e8d7b  (4 pub aliases)
```

### 9.4 Error System

Modules define and export error types like any other type:

```spore
-- src/billing/errors.spore

pub type BillingError {
    TaxCalculationFailed { region: Region, reason: Str },
    InvoiceGenerationFailed { order_id: OrderId },
    PaymentDeclined { amount: Money, code: ErrorCode },
}

pub type ValidationError {
    MissingField { field_name: Str },
    InvalidValue { field_name: Str, value: Str, expected: Str },
}
```

Error types follow the same visibility rules as all other types. A function's error list (`! TaxError | ValidationError`) must reference only error types that are visible from the function's module.

Cross-module error propagation:

```spore
-- src/api/handler.spore
import billing.invoice
import billing.errors

pub fn handle_create(req: Request) -> Response ! billing.errors.BillingError | ParseError
    uses [PaymentGateway, AuditLog]
{
    let order = parse_request(req)    -- may raise ParseError
    let invoice = billing.invoice.generate_invoice(order)  -- may raise BillingError
    Response.ok(invoice)
}
```

---

## 10. Package Structure

### 10.1 Package Manifest (`spore.toml`)

Every package has a `spore.toml` at its root:

```toml
[package]
name = "billing-lib"
version = "1.2.0"
type = "package"                # "package" | "application" | "platform"
description = "Invoice generation and tax calculation"
license = "MIT"
authors = ["Alice <alice@example.com>"]
spore-version = ">=0.5.0"      # minimum compiler version

[capabilities]
requires = ["PaymentGateway", "AuditLog"]

[dependencies]
json-parser = { version = "0.8.0", source = "https://packages.spore-lang.org/json-parser/0.8.0@sha256:e5f6a7b8..." }
decimal-math = { version = "2.1.0", source = "https://packages.spore-lang.org/decimal-math/2.1.0@sha256:c9d0e1f2..." }

[dev-dependencies]
test-framework = { version = "0.3.0", source = "https://packages.spore-lang.org/test-framework/0.3.0@sha256:1a2b3c4d..." }
```

### 10.2 Package Types

| Type | Has Platform? | Can Do IO? | Use Case |
|---|---|---|---|
| `package` | No | No (declares capability requirements) | Libraries, reusable components |
| `application` | Yes | Yes (via Platform) | Executables, services |
| `platform` | Is the Platform | Yes (raw syscalls) | Runtime providers |

### 10.3 Package Naming

- Package names are `kebab-case`: `billing-lib`, `json-parser`, `http-client`
- Package names are globally unique within a registry
- Module paths within a package use `snake_case` segments: `billing.invoice`, `json.parser`
- The package name and module path namespace are separate: package `billing-lib` contains module `billing.invoice`

### 10.4 Dependency Declaration

Dependencies are declared in `spore.toml` with version and content-addressed source:

```toml
[dependencies]
# Full form
json-parser = { version = "0.8.0", source = "https://packages.spore-lang.org/json-parser/0.8.0@sha256:e5f6a7b8..." }

# The @sha256:... suffix is the content hash of the package archive.
# This ensures reproducible builds — the exact bytes are verified.
```

### 10.5 Workspace Structure

For multi-package projects, a workspace `spore.toml` at the root:

```toml
# workspace spore.toml
[workspace]
members = [
    "packages/billing-lib",
    "packages/billing-api",
    "apps/invoice-service",
]
```

```
my-project/
├── spore.toml              -- workspace manifest
├── packages/
│   ├── billing-lib/
│   │   ├── spore.toml      -- package manifest
│   │   └── src/
│   │       └── billing/
│   │           ├── invoice.spore
│   │           └── types.spore
│   └── billing-api/
│       ├── spore.toml
│       └── src/
│           └── api/
│               └── handler.spore
└── apps/
    └── invoice-service/
        ├── spore.toml      -- application manifest (has platform)
        └── src/
            └── main.spore
```

---

## 11. Edge Cases

### 11.1 Empty Modules

An empty module is valid. It compiles, exports nothing, and has no capabilities:

```spore
-- src/billing/future.spore
-- (empty file)
```

```
$ sporec check src/billing/future.spore

[ok] billing.future
  exports: (none)
  capabilities: []
```

### 11.2 Modules with Only Types

A module can contain only type definitions and no functions. This is a common pattern for shared types:

```spore
-- src/billing/types.spore

pub type Invoice {
    id: InvoiceId,
    customer: Customer,
    line_items: Vec<LineItem>,
    total: Money,
    status: InvoiceStatus,
}

pub type InvoiceStatus {
    Draft,
    Sent,
    Paid,
    Void,
}

pub type LineItem {
    description: Str,
    quantity: I32,
    unit_price: Money,
    tax_rate: Decimal,
}
```

Type-only modules are pure by definition — they have no capabilities:

```
$ sporec check src/billing/types.spore

[ok] billing.types
  exports: Invoice, InvoiceStatus, LineItem
  capabilities: []
  cost: 0 (types only)
```

### 11.3 Re-Exports via Pub Aliases

The only re-export mechanism is `pub alias`. There is no implicit forwarding:

```spore
-- src/billing/shortcuts.spore
import billing.invoice
import billing.types

pub alias generate = billing.invoice.generate_invoice
pub alias Invoice = billing.types.Invoice
```

Alias chains are **not allowed**. A `pub alias` must point to an original definition, not to another alias:

```spore
-- src/billing/shortcuts.spore
pub alias gen = billing.invoice.generate_invoice    -- OK: points to original

-- src/api/shortcuts.spore
import billing.shortcuts
pub alias api_gen = billing.shortcuts.gen            -- ERROR: alias to alias
```

```
[error] alias chain detected at src/api/shortcuts.spore:3
  api_gen → billing.shortcuts.gen → billing.invoice.generate_invoice
  ─── aliases must point directly to original definitions, not to other aliases

  help: use `pub alias api_gen = billing.invoice.generate_invoice` instead
```

This restriction prevents unbounded alias chains and keeps the dependency graph simple for Agent analysis.

### 11.4 Diamond Dependencies

Diamond dependencies (where two modules depend on the same third module) are permitted and handled correctly:

```
       billing.types
        ↑        ↑
        │        │
  billing.invoice  billing.tax
        ↑        ↑
        │        │
       api.handler
```

The compiler ensures a single canonical version of `billing.types` is used throughout the dependency graph. The `.spore-lock` records both `sig` and `impl` hashes to guarantee consistency.

### 11.5 Forward References Within a Module

Within a single module, functions can reference each other regardless of declaration order:

```spore
-- src/billing/invoice.spore

pub fn generate_invoice(order: Order) -> Invoice ! TaxError {
    let tax = calculate_local_tax(order)    -- defined below: OK
    build_invoice(order, tax)               -- defined below: OK
}

fn calculate_local_tax(order: Order) -> TaxResult ! TaxError {
    ...
}

fn build_invoice(order: Order, tax: TaxResult) -> Invoice {
    ...
}
```

The compiler processes all declarations in a module before resolving references. Declaration order does not matter within a module.

However, forward references **across modules** are not allowed (this would imply circular dependencies).

### 11.6 Shadowing Rules

Module imports and aliases cannot shadow each other:

```spore
import billing.invoice as inv
import billing.inventory as inv    -- ERROR: 'inv' already used as module alias
```

```
[error] duplicate module alias at line 2
  'inv' is already used as an alias for billing.invoice (line 1)

  help: choose a different alias, e.g., `import billing.inventory as inventory`
```

Aliases cannot shadow imported module names:

```spore
import billing.invoice
alias invoice = billing.types.Invoice    -- ERROR: 'invoice' conflicts with module name
```

```
[error] name conflict at line 2
  'invoice' conflicts with imported module billing.invoice (line 1)

  help: choose a different alias name, e.g., `alias Inv = billing.types.Invoice`
```

### 11.7 Modules with Holes and Exports

A module with holes can still be imported and its types used. Only function **calls** at runtime are blocked:

```spore
-- src/billing/invoice.spore (has holes)
pub fn generate_invoice(order: Order) -> Invoice ! TaxError {
    ?invoice_logic
}

-- src/api/handler.spore
import billing.invoice

-- This compiles fine. handler is transitively partial.
pub fn handle(req: Request) -> Response ! ApiError {
    let inv = billing.invoice.generate_invoice(parse_order(req))
    Response.ok(inv)
}
```

```
$ sporec check

[partial] billing.invoice.generate_invoice
  holes: ?invoice_logic

[partial] api.handler.handle
  depends on partial: billing.invoice.generate_invoice

Build: success (2 partial functions)
```

---

## 12. Design Rationale

### 12.1 File = Module (from Elm, Go, Koka, Zig)

**Decision**: One `.spore` file = one module. No separate `mod` declarations.

**Why**: Elm, Go, Koka, and Zig all demonstrate that file-based module identity is the simplest approach that scales. It eliminates the confusion of Rust's `mod` vs `use` distinction and the boilerplate of OCaml's separate `.mli` files.

**Trade-off**: Very large modules must be split into multiple files/modules. We accept this as a feature, not a bug — it encourages smaller, focused modules.

**Reference**: Elm proves this works for large codebases. Go proves it works at Google scale. Zig's file=struct insight shows the concept can be taken further (we stop at file=module).

### 12.2 Content-Addressed Functions — Dual Hash (inspired by Unison)

**Decision**: Every function carries two hashes: a **signature hash (`sig`)** for API compatibility, and an **implementation AST hash (`impl`)** for exact content addressing. Partial functions have `impl = None`.

**Why**: Unison proved that content-addressing eliminates an entire class of dependency problems. But Unison's full content-addressing (hashing the AST) abandons file-based tooling. A single signature-only hash preserves file-based workflows but cannot pin exact implementations for reproducible builds. The dual-hash scheme gives us both:

- `sig` provides the stability guarantee — hole filling and body refactoring never break downstream code, and breaking changes (`sig` changed) require explicit `spore --permit`.
- `impl` provides deterministic content addressing — `.spore-lock` can pin exact code for reproducible builds, cache invalidation, and auditing.

An earlier design considered tracking only a "last implementation" concept (e.g., "did the implementation change since last build?"), but this was rejected as too fuzzy and non-deterministic — it depends on build ordering and mutable state. The AST hash is computed directly from the compiled representation and is unambiguous: two functions with the same `impl` hash have identical behavior, period.

**Trade-off**: Signature changes still break dependents (requiring `--permit`). This is intentional — signature changes are semantic changes that **should** require explicit acknowledgment. Implementation-only changes update `impl` silently — no downstream action required.

**Reference**: Unison's content-addressing is the gold standard. Roc's content-hashed package URLs provide a precedent for the registry side. Go's `go.sum` shows that content hashing is mainstream. Nix's derivation hashes demonstrate the value of pinning exact content for reproducibility.

### 12.3 Visibility: Private / pub(pkg) / pub (from Rust, Go)

**Decision**: Three visibility levels — private (default), `pub(pkg)`, and `pub`.

**Why**: The cross-language research (§ Cross-Cutting Observations) shows that most languages start with two levels and later add a third. Go added `internal/`. Rust has `pub(crate)`. Idris 2 has three levels. Starting with three levels avoids the need for a backwards-compatible extension later.

**Trade-off**: Slightly more complex than Elm's two-level system. But `pub(pkg)` is a common real-world need: internal helpers shared across modules within a package but not exposed to external consumers.

**Why not four+ levels (Rust's `pub(super)`, `pub(in path)`)?**: The research shows that fine-grained visibility (`pub(in path)`) is rarely used and adds cognitive load. Three levels cover 99% of use cases.

### 12.4 No Circular Dependencies (from Elm, Go, Roc)

**Decision**: Circular module dependencies are a compile error.

**Why**: Nearly every language in the research survey forbids cycles. Elm, Go, Roc, Koka, and Zig all enforce this. The benefits are well-established:

- Predictable compilation order (topological sort)
- Clean architecture (forces extraction of shared types)
- Simpler tooling (no fixpoint computation for module types)
- Agent-friendly (DAG is easier to traverse than arbitrary graphs)

**Trade-off**: Occasional friction when two modules are tightly coupled. The resolution (extract shared types into a third module) is a well-known pattern from the Elm community.

**Reference**: Elm's community has lived with this constraint for years and considers it beneficial. Go's experience at Google scale validates that cycle-free packages produce maintainable codebases.

### 12.5 Platform System (from Roc)

**Decision**: A Platform provides all IO capabilities. Pure packages cannot perform IO.

**Why**: Roc demonstrated the strongest supply-chain security model of any language: packages are pure by default, and only the Platform can do IO. This is a direct application of the principle of least privilege.

Spore integrates this with its existing capability system: a function declaring `uses [FileRead]` means "I need a Platform that provides FileRead." The Platform is the sole authority for fulfilling capability requirements at runtime.

**Trade-off**: Platform authoring is more complex than a simple `main()` function. Creating a new Platform requires implementing capability providers. In practice, most developers will use community Platforms and never need to write one.

**Reference**: Roc's platform/package separation. The capability-based security literature. The POLA (Principle of Least Authority) design philosophy.

### 12.6 No Functors / No Parameterized Modules (contra OCaml)

**Decision**: No functors. Generics and capabilities replace the need.

**Why**: OCaml/SML functors are the most powerful module system in any language, but they add a separate "module language" that doubles the conceptual surface area. The research survey found that every language except OCaml/SML achieves similar results through generics, typeclasses, comptime evaluation, or dependent types.

Spore uses generics (`where T: Constraint`) for type-level parameterization and capabilities (`uses [...]`) for effect-level parameterization. Together, they cover the use cases that functors address (e.g., a `Set` parameterized over an `Ord` type) without a separate module language.

**Trade-off**: Some patterns that are natural with functors (e.g., generating an entire module of functions parameterized over a type) require slightly more boilerplate in a generics-based system.

**Reference**: Rust's decision to use generics instead of functors. Haskell's use of typeclasses. The observation from the research that "languages that integrate parameterization into the core type system seem to achieve similar results with less conceptual overhead."

### 12.7 No Module-Level Capability Ceiling

**Decision**: Spore has no module-level capability carrier and no module-level capability ceiling.

**Why**: File paths fully determine module identity, while capability checking stays attached to function signatures and package / Platform configuration. This keeps authority explicit at call boundaries and avoids hidden module metadata.

### 12.8 Import/Alias Separation (novel)

**Decision**: `import` is for modules. `alias` is for items. No overlap.

**Why**: Many languages conflate module imports and item imports, leading to confusion. Rust has `mod` (declare) vs `use` (import). Haskell has `import qualified` vs `import ... exposing`. Python has `import module` vs `from module import item`.

Spore separates the two concepts completely:

- `import billing.invoice` — makes the module accessible by qualified name
- `import billing.invoice as inv` — renames the module for local use
- `alias gen = billing.invoice.generate_invoice` — binds a specific item to a local name

This eliminates ambiguity: `import` always operates on modules, `alias` always operates on items. An Agent parsing imports can unambiguously determine whether a name refers to a module or an item.

**Reference**: Haskell's qualified imports for the module aliasing side. Elm's explicit exposing lists for the item side. The insight: keep them separate rather than trying to unify them.

---

## Appendix A: CLI Command Reference

### Module-Related Commands

```
sporec check [path]           Check modules for errors
sporec check --show-deps      Show dependency graph
sporec build                  Build all modules in topological order
sporec build --show-order     Show build order
sporec --holes [path]         List all holes across modules
sporec --fixes [path]         Apply compiler-provided source fixes (never writes module capability metadata)
sporec fmt [path]             Format source, including import ordering
```

### Snapshot & Lock Commands

```
spore snapshot                Show package/module/function hashes (sig + impl)
spore snapshot --show <mod>   Show specific module's interface hash and per-function sig/impl
spore --permit <function>     Accept a sig change, update .spore-lock
spore --permit --all          Accept all pending sig changes
spore lock --verify           Verify .spore-lock sig and impl match current state
```

### Audit & Inspection Commands

```
spore audit <package>         Audit a package's capability usage
spore deps <module>           Show a module's dependency tree
spore deps --reverse <module> Show what depends on a module
spore cost-report <module>    Show cost summary for a module
spore exports <module>        List a module's public API
```

### Example Workflow

```bash
# 1. Create a new module
$ touch src/billing/refund.spore

# 2. Write code (editor)

# 3. Check for errors
$ sporec check src/billing/refund.spore

# 4. Review function-level capabilities in signatures

# 5. Check again — clean
$ sporec check src/billing/refund.spore

# 6. View the module's public API
$ spore exports billing.refund

# 7. View the full dependency graph
$ sporec check --show-deps

# 8. Build
$ sporec build

# 9. If a signature changed, review and accept
$ spore --permit billing.refund.process_refund
```

---

## Appendix B: Grammar Summary

```
-- No in-file module header; module path comes from file location

-- Imports
import_decl   ::= 'import' module_path ('as' IDENT)?
alias_decl    ::= 'alias' IDENT '=' qualified_item

-- Visibility modifiers
visibility    ::= 'pub' | 'pub(pkg)'

-- Declarations
fn_decl       ::= visibility? 'fn' IDENT generics? '(' params ')' '->' type ('!' error_list)?
                   cost_clause? where_clause? uses_clause? block
-- with_clause removed in v0.4 — properties are now auto-inferred from `uses`.
cost_clause   ::= 'cost' '≤' NUMBER
where_clause  ::= 'where' constraint (',' constraint)*
uses_clause   ::= 'uses' capability_list
effect_list   ::= '[' IDENT (',' IDENT)* ']'
type_decl     ::= visibility? 'type' IDENT generics? type_body
alias_item    ::= visibility? 'alias' IDENT '=' qualified_item

-- Paths
module_path   ::= IDENT ('.' IDENT)*
qualified_item::= module_path '.' IDENT
capability_list ::= '[' IDENT (',' IDENT)* ']'

-- Identifiers
IDENT         ::= [a-zA-Z_][a-zA-Z0-9_]*
```

---

## Appendix C: Design Decisions Summary

| # | Decision | Status | Key Inspiration |
|---|---|---|---|
| 1 | File = Module | **Confirmed** | Elm, Go, Zig |
| 2 | Content-Addressed Functions (dual hash: `sig` + `impl`) | **Confirmed** | Unison |
| 3 | Visibility: private / pub(pkg) / pub | **Confirmed** | Rust, Go |
| 4 | No Circular Dependencies | **Confirmed** | Elm, Go, Roc |
| 5 | Platform System | **Confirmed** | Roc |
| 6 | No Functors | **Confirmed** | Rust, Haskell (alternatives) |
| 7 | Module-Level Capability Ceiling | **Confirmed** | Novel (Koka-influenced) |
| 8 | Import/Alias Separation | **Confirmed** | Novel (Haskell, Elm influenced) |

---

*End of Module System Design Document v0.1*
