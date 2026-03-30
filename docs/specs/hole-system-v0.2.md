# Spore Hole System — Design Document v0.2

> **Status**: Draft  
> **Scope**: `sporec` compiler, `spore` Codebase Manager, Agent workflow  
> **Depends on**: Cost system, Capability system, Snapshot system, Error system

---

## 0. Guiding Principles

| Principle | Implication |
|---|---|
| **Holes are valid states, not errors** | A program with holes compiles successfully. It is *partial*, not *broken*. |
| **Compiler-first** | All hole information is available via `sporec` CLI output—no IDE mode required. |
| **Information-self-sufficient** | A HoleReport contains everything needed to fill the hole: types, bindings, capabilities, candidates, cost budget, pending errors. An Agent reading the report needs zero additional context. |
| **Signatures are gravity centers** | Holes live in *implementations*, never in signatures. A function's signature hash is unchanged by holes inside its body. |

---

## 1. Hole Syntax

### 1.1 Named Holes

Every hole has a name. Anonymous holes are forbidden—naming is how humans and Agents refer to a specific gap.

```spore
fn calculate_tax(income: Money, region: Region) -> Money ! [InvalidRegion]
where
    effects: pure, deterministic
    cost ≤ 500
    uses [TaxTable]
{
    ?tax_logic
}
```

The name `tax_logic` is project-unique within its module. The compiler rejects duplicate hole names in the same module.

### 1.2 Typed Holes

When the developer wants to constrain a hole beyond what the compiler infers, an explicit type annotation is allowed:

```spore
fn format_report(data: Vec<Transaction>) -> String ! []
where
    effects: pure
    cost ≤ 1000
{
    let header = build_header(data)
    let body: String = ?report_body : String    -- explicit type on hole
    let footer = build_footer(data)
    header ++ body ++ footer
}
```

If the annotation conflicts with the inferred type, the compiler emits a `hole-type-conflict` diagnostic (warning, not error—holes are exploratory by nature).

### 1.3 Multiple Holes in One Function

A function may contain any number of holes. Each independently named, independently fillable.

```spore
fn reconcile(ledger: Ledger, transactions: Vec<Tx>) -> Ledger ! [Mismatch]
where
    effects: pure
    cost ≤ 5000
{
    let grouped = group_by_account(transactions)
    let adjustments = ?compute_adjustments
    let validated   = ?validate_adjustments
    apply(ledger, validated)
}
```

### 1.4 Nested Holes (Holes Inside Expressions)

Holes can appear anywhere an expression is expected, including inside function arguments, match arms, and let bindings:

```spore
fn render(page: Page) -> Html ! [TemplateError]
where
    effects: pure
    cost ≤ 800
    uses [Templates]
{
    let nav = render_nav(?nav_items)                 -- hole as argument
    let content = match page.kind {
        Article(a) => render_article(a),
        Gallery(g) => ?gallery_renderer,               -- hole in match arm
        _          => render_fallback(page),
    }
    compose(nav, content)
}
```

The compiler infers:
- `?nav_items` must have the type that `render_nav` expects as its first parameter.
- `?gallery_renderer` must have the same type as the other match arms (here `Html ! [TemplateError]`).

### 1.5 Syntax Grammar (Formal)

```
hole       ::= '?' IDENT (':' type)?
IDENT      ::= [a-z_][a-zA-Z0-9_]*
```

Holes are expressions. They can appear in any expression context. They cannot appear in:
- Type positions (see §8.2 for *type holes*, a separate concept)
- Signature positions (parameter types, return types, error lists, `where` clauses)
- Module-level declarations (a hole cannot replace a function declaration)

---

## 2. Hole Semantics

### 2.1 Type of a Hole

A hole's type is the **intersection** of all constraints imposed by its context:

```spore
fn example(x: Int, y: String) -> Bool ! []
where effects: pure
{
    let a: Int = ?h1         -- type of ?h1 is Int (from let binding)
    let b = if a > 0 {
        ?h2                  -- type of ?h2 is Bool (from return type,
    } else {                 --   since this branch determines the return)
        false
    }
    b
}
```

If context provides no constraints, the hole is *unconstrained*—the compiler reports its type as `_` and lists available bindings so the Agent can propose a type.

### 2.2 Partial Functions

A function containing at least one hole is **partial**. Partial functions:

| Property | Complete Function | Partial Function |
|---|---|---|
| Can be compiled | ✓ | ✓ |
| Can be called at runtime | ✓ | ✗ |
| Can be simulated | ✓ | ✓ |
| Appears in module exports | ✓ | ✓ (marked `partial`) |
| Signature hash changes on edit | if signature changes | never (holes are body) |
| Cost is fully determined | ✓ | ✗ (lower bound only) |

A caller that invokes a partial function becomes partial itself, transitively. The compiler tracks this:

```spore
fn outer() -> Int ! []
where effects: pure, cost ≤ 1000
{
    inner() + 1    -- outer is now partial because inner is partial
}

fn inner() -> Int ! []
where effects: pure, cost ≤ 200
{
    ?placeholder
}
```

```
$ sporec check src/example.spore

[partial] inner : () -> Int ! []
  holes: ?placeholder

[partial] outer : () -> Int ! []
  depends on partial: inner
```

### 2.3 Holes and Pattern Matching

#### Hole in a branch arm

If a hole appears in one branch of a `match`, other branches can still execute normally during simulation:

```spore
fn describe(shape: Shape) -> String ! []
where effects: pure
{
    match shape {
        Circle(r)    => "circle with radius " ++ show(r),
        Rectangle(w, h) => ?describe_rect,
        Triangle(a, b, c) => ?describe_tri,
    }
}
```

Simulating with `shape = Circle(5.0)` returns `"circle with radius 5.0"` normally.
Simulating with `shape = Rectangle(3.0, 4.0)` reaches `?describe_rect` and emits a HoleReport.

#### Case-splitting on a hole

If the scrutinee of a `match` is itself a hole, the compiler reports **all branches** as blocked but still infers the hole's type from usage:

```spore
fn classify(data: RawData) -> Category ! [ParseError]
where effects: pure
{
    match ?parsed_data {         -- hole as scrutinee
        Valid(v)   => categorize(v),
        Invalid(e) => raise ParseError(e),
    }
}
```

The compiler infers: `?parsed_data` must have a type with at least the variants `Valid(_)` and `Invalid(_)`. If a type in scope matches (e.g., `Result<ValidData, ErrorInfo>`), it's reported as the inferred type.

### 2.4 Filling Order

The compiler does not require or accept explicit priority annotations. Instead, it analyzes **dependencies between holes**: if hole A's output feeds into hole B's context (e.g., as a binding or argument), then A should be filled before B. The compiler builds a dependency graph and recommends a topological filling order via `spore holes --suggest-order`.

**Ranking heuristic: transitive dependents count.** Within the same topological tier, holes are ranked by how many other holes (transitively) depend on them. A hole with more dependents is more "impactful"—filling it unlocks more downstream work.

Example:
```
E1 depends on C, D
E2 depends on C
D  depends on A
C  depends on B
```

Transitive dependents:
- B → {C, E1, E2} = 3   (highest impact)
- A → {D, E1} = 2
- C → {E1, E2} = 2
- D → {E1} = 1
- E1, E2 → {} = 0

Suggested order: B → A = C → D → E1 = E2

When no dependency relationship exists between two holes, they are independent and may be filled in any order. The developer or Agent is free to override the suggested order at any time—it is advisory, not enforced.

---

## 3. Simulated Execution (Partial Run)

### 3.1 Execution Model

`sporec` performs **compile-time abstract interpretation** (simulated execution). For complete functions this produces cost information. For partial functions, it additionally produces **HoleReports** at each point where a hole is reached.

Simulation is *path-sensitive*: each branch of a conditional or match is explored independently. A hole in one path does not block simulation of other paths.

### 3.2 HoleReport Structure

When simulation reaches a hole, it emits:

```json
{
  "schema": "spore/hole-report/v1",
  "hole": {
    "name": "payment_logic",
    "location": {
      "file": "src/billing/charge.spore",
      "line": 42,
      "column": 5
    },
    "dependencies": []
  },
  "type": {
    "expected": "ChargeResult ! [PaymentFailed, GatewayTimeout]",
    "inferred_from": "return position of fn charge_customer"
  },
  "bindings": [
    {
      "name": "customer",
      "type": "Customer",
      "simulated_value": { "kind": "symbolic", "origin": "parameter" }
    },
    {
      "name": "amount",
      "type": "Money",
      "simulated_value": { "kind": "symbolic", "origin": "parameter" }
    },
    {
      "name": "validated_card",
      "type": "Card",
      "simulated_value": {
        "kind": "computed",
        "expression": "validate_card(customer.card)",
        "origin": "let binding, line 38"
      }
    }
  ],
  "capabilities": ["PaymentGateway", "AuditLog"],
  "errors_to_handle": ["PaymentFailed", "GatewayTimeout"],
  "cost": {
    "budget_total": 2000,
    "cost_before_hole": 340,
    "budget_remaining": 1660
  },
  "candidates": [
    {
      "function": "gateway_charge",
      "signature": "(Card, Money) -> ChargeResult ! [PaymentFailed, GatewayTimeout]",
      "match_quality": "exact",
      "requires_capabilities": ["PaymentGateway"],
      "estimated_cost": 800
    },
    {
      "function": "retry_charge",
      "signature": "(Card, Money, RetryPolicy) -> ChargeResult ! [PaymentFailed, GatewayTimeout]",
      "match_quality": "partial (needs RetryPolicy)",
      "requires_capabilities": ["PaymentGateway"],
      "estimated_cost": 1500
    }
  ],
  "dependent_holes": [],
  "enclosing_function": {
    "name": "charge_customer",
    "signature": "(Customer, Money) -> ChargeResult ! [PaymentFailed, GatewayTimeout]",
    "effects": ["deterministic"],
    "full_cost_budget": 2000
  }
}
```

### 3.3 Field Reference

| Field | Description |
|---|---|
| `hole.name` | Developer-assigned name |
| `hole.location` | Source location (file, line, column) |
| `hole.dependencies` | Names of other holes whose output feeds into this hole's context. Empty if the hole has no upstream hole dependencies. Used by the compiler to suggest a topological filling order. |
| `type.expected` | The type this hole must produce, including error variants |
| `type.inferred_from` | Human-readable explanation of *why* this type is expected |
| `bindings` | All variables in scope at the hole site. Each has a name, type, and simulated value. Simulated values are either `symbolic` (no concrete value—came from a parameter or external call) or `computed` (derived from prior simulation steps). |
| `capabilities` | The `uses` list from the enclosing function—what the hole is allowed to call |
| `errors_to_handle` | Error types declared in the enclosing function's `!` list that are not yet handled before the hole |
| `cost.budget_total` | The `cost ≤ N` from the enclosing function's `where` clause |
| `cost.cost_before_hole` | Cumulative cost of statements executed before reaching this hole |
| `cost.budget_remaining` | `budget_total - cost_before_hole`. The filling must fit within this. |
| `candidates` | Functions in scope whose return type is compatible with the hole's expected type. Includes match quality, required capabilities, and estimated cost. |
| `dependent_holes` | Other holes that would become reachable if this hole is filled (currently blocked by this hole's path). |
| `enclosing_function` | The full signature context of the containing function. |

### 3.4 Multi-Path Simulation

```spore
fn route(request: Request) -> Response ! [NotFound, Unauthorized]
where
    effects: deterministic
    cost ≤ 3000
    uses [Auth, Database]
{
    match request.method {
        GET  => handle_get(request.path),
        POST => ?handle_post,
        PUT  => ?handle_put,
        _    => Response.method_not_allowed(),
    }
}
```

Running `sporec --simulate route`:

```
Simulating route(request: Request)...

Path 1: request.method = GET
  → completes normally via handle_get
  → cost: 450

Path 2: request.method = POST
  → reaches hole ?handle_post at line 12
  → HoleReport emitted (see --query-hole ?handle_post)
  → cost before hole: 80

Path 3: request.method = PUT
  → reaches hole ?handle_put at line 13
  → HoleReport emitted (see --query-hole ?handle_put)
  → cost before hole: 80

Path 4: request.method = _
  → completes normally via Response.method_not_allowed
  → cost: 20

Summary:
  complete paths: 2/4
  holes encountered: ?handle_post, ?handle_put
  cost range: [20, 450] (complete paths only)
```

### 3.5 Cost Accounting Rules

1. **Code before a hole**: cost is counted normally.
2. **The hole itself**: cost = 0. The hole is a placeholder, not an operation.
3. **Code after a hole** (in the same sequential block): cost is counted, but the simulated value flowing from the hole is `symbolic`. Functions called on symbolic values have their cost estimated from their declared `cost ≤` bound (worst-case).
4. **Budget remaining**: reported so the Agent knows how much room is left.

```spore
fn pipeline(data: Vec<Record>) -> Summary ! [DataError]
where
    effects: pure
    cost ≤ 10000
{
    let cleaned = clean(data)          -- cost: 200
    let transformed = ?transform_step  -- cost: 0 (hole), budget remaining: 9800
    let aggregated = aggregate(transformed)  -- cost: estimated from aggregate's declared bound
    aggregated
}
```

---

## 4. Hole Lifecycle

### 4.1 Creation

A developer writes a hole to mark an unfinished piece of logic:

```spore
fn send_notification(user: User, event: Event) -> NotifyResult ! [DeliveryFailed]
where
    effects: deterministic
    cost ≤ 500
    uses [EmailService, PushService]
{
    let channel = select_channel(user.preferences, event.urgency)
    ?dispatch_notification
}
```

The compiler accepts this. The function is marked `partial`.

### 4.2 Discovery

```
$ sporec --holes src/

Holes in project:

  ?dispatch_notification
    in: send_notification (src/notify.spore:8)
    type: NotifyResult ! [DeliveryFailed]
    capabilities: [EmailService, PushService]

  ?transform_step
    in: pipeline (src/etl.spore:14)
    type: Vec<TransformedRecord> (inferred)
    capabilities: []

  ?handle_post
    in: route (src/server.spore:12)
    type: Response ! [NotFound, Unauthorized]
    capabilities: [Auth, Database]

  ?handle_put
    in: route (src/server.spore:13)
    type: Response ! [NotFound, Unauthorized]
    capabilities: [Auth, Database]

Total: 4 holes across 3 functions
```

### 4.3 Filling

A developer (or Agent) replaces the hole with an implementation:

```spore
-- Before:
    ?dispatch_notification

-- After:
    match channel {
        Email => email_service.send(user.email, event.to_email_body()),
        Push  => push_service.send(user.device_id, event.to_push_body()),
    }
```

### 4.4 Verification

After filling, the compiler re-checks:

```
$ sporec check src/notify.spore

[ok] send_notification : (User, Event) -> NotifyResult ! [DeliveryFailed]
  cost: 420 (within budget of 500)
  all holes filled ✓
```

If the filling is incorrect:

```
$ sporec check src/notify.spore

[error] send_notification : (User, Event) -> NotifyResult ! [DeliveryFailed]
  line 10: type mismatch
    expected: NotifyResult ! [DeliveryFailed]
    found:    String
  holes remaining: 0 (but function has errors)
```

### 4.5 Partial Filling (Revealing Deeper Holes)

Filling one hole may reveal new holes that were previously unreachable:

```spore
fn process(input: RawData) -> Output ! [ProcessError]
where effects: pure, cost ≤ 5000
{
    let parsed = ?parse_step
    let result = transform(parsed)    -- transform itself contains ?transform_inner
    result
}
```

Before filling `?parse_step`, the compiler cannot reach `transform`'s body (the argument is symbolic). After filling:

```spore
    let parsed = parse_raw(input)     -- filled!
```

Now `transform(parsed)` can be simulated deeper, potentially surfacing `?transform_inner` in `transform`'s body.

```
$ sporec --holes src/

Holes in project:

  ?transform_inner
    in: transform (src/transform.spore:5)
    type: TransformedData (inferred)
    revealed by: filling of ?parse_step

Total: 1 hole (was 1 before, now different hole)
```

---

## 5. Hole Queries (CLI)

### 5.1 `sporec --holes`

Lists all holes in the project or in specified files.

```
$ sporec --holes [FILE|DIR...]
$ sporec --holes --json              # machine-readable output
$ sporec --holes --module billing    # filter by module
```

**Human-readable output:**

```
$ sporec --holes

  ?charge_payment     in process_order  (src/orders.spore:18)   Response ! [PaymentFailed]
  ?handle_post        in route          (src/server.spore:12)   Response ! [NotFound, Unauthorized]
  ?dispatch_notif     in send_notif     (src/notify.spore:8)    NotifyResult ! [DeliveryFailed]
  ?reserve_stock      in process_order  (src/orders.spore:17)   ReserveResult ! [OutOfStock]
  ?handle_put         in route          (src/server.spore:13)   Response ! [NotFound, Unauthorized]
  ?transform_step     in pipeline       (src/etl.spore:14)      Vec<TransformedRecord>

Total: 6 holes, 4 functions affected
```

**JSON output:**

```
$ sporec --holes --json

{
  "holes": [
    {
      "name": "charge_payment",
      "location": { "file": "src/orders.spore", "line": 18, "column": 5 },
      "enclosing_function": "process_order",
      "expected_type": "Response ! [PaymentFailed]",
      "capabilities": ["Inventory", "PaymentGateway"],
      "dependencies": []
    },
    ...
  ],
  "summary": {
    "total_holes": 6,
    "functions_affected": 4
  }
}
```

### 5.2 `sporec --query-hole`

Returns the full HoleReport for a specific hole:

```
$ sporec --query-hole ?charge_payment
```

Outputs the complete JSON structure described in §3.2. This is the primary interface for Agents.

If the hole has not been simulated yet, `sporec` runs simulation automatically and then outputs the report.

### 5.3 `sporec --simulate`

Runs simulated execution of a specific function and reports all paths:

```
$ sporec --simulate process_order
$ sporec --simulate process_order --path 2    # simulate only path 2
$ sporec --simulate process_order --json       # machine-readable
$ sporec --simulate process_order --with 'order = Order { ... }'  # provide concrete input
```

### 5.4 `spore` (Codebase Manager) Integration

The stateful `spore` tool wraps `sporec` and adds project-level intelligence:

```
$ spore holes                         # like sporec --holes, but project-aware
$ spore holes --suggest-order         # topological ordering based on dependency analysis
$ spore fill ?charge_payment          # opens Agent workflow for this hole
$ spore fill --all                    # Agent fills all holes in dependency order
```

`spore holes --suggest-order` analyzes inter-hole dependencies, ranks by transitive dependents count (impact), and recommends a filling order:

```
$ spore holes --suggest-order

Suggested filling order (by dependency + impact):

  1. ?validate_items     (3 dependents: ?compute_tax, ?format_invoice, ?generate_pdf)
  2. ?compute_tax        (1 dependent: ?format_invoice)
  3. ?handle_post        (0 dependents — independent)
  4. ?handle_put         (0 dependents — independent)
  5. ?dispatch_notif     (0 dependents — independent)
  6. ?transform_step     (0 dependents — independent)

Holes 3–6 are independent; fill in any order.
```

---

## 6. Interaction with Other Systems

### 6.1 Cost System

| Aspect | Behavior |
|---|---|
| Hole cost | Always 0 |
| Code before hole | Normal cost |
| Code after hole (using hole's symbolic output) | Estimated via callee's declared cost bound |
| Cost report for partial function | Reports a **range**: `[known_minimum, known_minimum + sum_of_remaining_budgets]` |
| Budget remaining at hole site | Included in HoleReport |

Example:

```spore
fn pipeline(xs: Vec<Int>) -> Vec<Int> ! []
where effects: pure, cost ≤ 1000
{
    let a = sort(xs)          -- cost: 200
    let b = ?middle_step      -- cost: 0, budget_remaining: 800
    let c = deduplicate(b)    -- cost: estimated ≤ 150 (from deduplicate's declared bound)
    c
}
```

```
$ sporec check src/pipeline.spore

[partial] pipeline: (Vec<Int>) -> Vec<Int> ! []
  known cost: 200 (before hole) + ≤150 (after hole) = ≤350
  budget for ?middle_step: ≤650 (1000 - 350)
  note: ?middle_step filling must have cost ≤ 650
```

The budget constraint propagates into the HoleReport so the Agent knows it cannot propose a filling with cost > 650.

### 6.2 Capability System

Holes inherit the enclosing function's `uses` list. A filling can only call functions whose required capabilities are a subset of the available set.

```spore
fn sync_data(src: Database, dst: Database) -> SyncResult ! [SyncError]
where
    effects: deterministic
    cost ≤ 10000
    uses [DatabaseRead, DatabaseWrite, AuditLog]
{
    let diff = compute_diff(src, dst)   -- requires [DatabaseRead]
    ?apply_changes         -- may use [DatabaseRead, DatabaseWrite, AuditLog]
}
```

The HoleReport for `?apply_changes` lists `capabilities: ["DatabaseRead", "DatabaseWrite", "AuditLog"]`. Candidate functions that require capabilities outside this set are excluded.

A filling that attempts to use an unlisted capability produces a compile error:

```
[error] apply_changes filling uses [NetworkAccess] which is not in
        sync_data's declared uses [DatabaseRead, DatabaseWrite, AuditLog]
```

### 6.3 Snapshot System

**Fundamental rule**: holes are implementation details. They do not affect the function's signature hash.

| Action | Signature hash changes? |
|---|---|
| Add a hole to body | No |
| Remove a hole (fill it) | No |
| Rename a hole | No |
| Change parameter types | Yes |
| Change return type | Yes |
| Change error list | Yes |
| Change `uses` list | Yes |

This means:
- A function's snapshot is stable while its implementation is being developed through holes.
- Downstream dependents are not invalidated when an upstream function goes from partial → complete.
- `spore` can cache and reuse snapshots across hole-filling iterations.

### 6.4 Error System

The enclosing function's declared error list (`! [Err1, Err2]`) flows into the hole. The HoleReport includes `errors_to_handle`—the subset of declared errors not already handled by code before the hole.

```spore
fn fetch_and_parse(url: Url) -> Document ! [NetworkError, ParseError, Timeout]
where
    effects: deterministic
    cost ≤ 3000
    uses [Http]
{
    let response = http_get(url)          -- may raise NetworkError, Timeout
        |> catch Timeout => retry_once(url)  -- Timeout handled here
    ?parse_response
}
```

HoleReport for `?parse_response`:
```json
{
  "errors_to_handle": ["ParseError"],
  "errors_already_handled": ["Timeout"],
  "errors_passthrough": ["NetworkError"]
}
```

- `errors_to_handle`: the filling should handle or propagate `ParseError`.
- `errors_already_handled`: `Timeout` was handled before the hole; the filling doesn't need to worry about it.
- `errors_passthrough`: `NetworkError` can propagate upward—the filling may also propagate it but doesn't need explicit handling.

---

## 7. Agent Workflow Example

A complete end-to-end workflow showing Human–Agent–Compiler interaction.

### Step 1: Human Writes a Function with Holes

```spore
-- src/billing/invoice.spore

fn generate_invoice(
    customer: Customer,
    items: Vec<LineItem>,
    tax_region: TaxRegion,
) -> Invoice ! [TaxCalculationError, InvalidLineItem]
where
    effects: pure, deterministic
    cost ≤ 5000
    uses [TaxTable]
{
    let validated_items = ?validate_items
    let subtotal = sum(validated_items.map(|i| i.price * i.quantity))
    let tax = ?compute_tax
    let total = subtotal + tax
    Invoice.new(customer, validated_items, subtotal, tax, total)
}
```

### Step 2: Agent Queries the Holes

```
$ sporec --query-hole ?validate_items --json
```

```json
{
  "schema": "spore/hole-report/v1",
  "hole": {
    "name": "validate_items",
    "location": { "file": "src/billing/invoice.spore", "line": 12, "column": 5 },
    "dependencies": []
  },
  "type": {
    "expected": "Vec<LineItem>",
    "inferred_from": "used as argument to .map() on line 13, and passed to Invoice.new on line 16"
  },
  "bindings": [
    { "name": "customer", "type": "Customer", "simulated_value": { "kind": "symbolic", "origin": "parameter" } },
    { "name": "items", "type": "Vec<LineItem>", "simulated_value": { "kind": "symbolic", "origin": "parameter" } },
    { "name": "tax_region", "type": "TaxRegion", "simulated_value": { "kind": "symbolic", "origin": "parameter" } }
  ],
  "capabilities": ["TaxTable"],
  "errors_to_handle": ["InvalidLineItem"],
  "cost": {
    "budget_total": 5000,
    "cost_before_hole": 0,
    "budget_remaining": 5000
  },
  "candidates": [
    {
      "function": "validate_line_items",
      "signature": "(Vec<LineItem>) -> Vec<LineItem> ! [InvalidLineItem]",
      "match_quality": "exact",
      "requires_capabilities": [],
      "estimated_cost": 300
    },
    {
      "function": "filter_valid",
      "signature": "(Vec<LineItem>) -> Vec<LineItem> ! []",
      "match_quality": "partial (does not raise InvalidLineItem)",
      "requires_capabilities": [],
      "estimated_cost": 150
    }
  ],
  "dependent_holes": ["compute_tax"],
  "enclosing_function": {
    "name": "generate_invoice",
    "signature": "(Customer, Vec<LineItem>, TaxRegion) -> Invoice ! [TaxCalculationError, InvalidLineItem]",
    "effects": ["pure", "deterministic"],
    "full_cost_budget": 5000
  }
}
```

### Step 3: Agent Generates a Filling

The Agent reads the HoleReport and proposes:

```spore
-- Filling for ?validate_items
validate_line_items(items)
```

Reasoning (Agent-internal):
- `validate_line_items` has an exact type match.
- It raises `InvalidLineItem`, which is in the enclosing function's error list.
- Its cost (300) fits within the budget (5000).
- It requires no extra capabilities.

### Step 4: Compiler Verifies the Filling

```
$ sporec check src/billing/invoice.spore

[partial] generate_invoice : (Customer, Vec<LineItem>, TaxRegion) -> Invoice ! [TaxCalculationError, InvalidLineItem]
  ?validate_items: filled ✓ (cost 300)
  ?compute_tax: still open
  known cost: 300 + 120 (sum, map, Invoice.new) = 420
  budget for ?compute_tax: ≤4580
```

### Step 5: Agent Fills the Next Hole

```
$ sporec --query-hole ?compute_tax --json
```

```json
{
  "hole": { "name": "compute_tax", "dependencies": ["validate_items"] },
  "type": {
    "expected": "Money",
    "inferred_from": "used in arithmetic (subtotal + tax) on line 15"
  },
  "bindings": [
    { "name": "customer", "type": "Customer", "simulated_value": { "kind": "symbolic" } },
    { "name": "items", "type": "Vec<LineItem>", "simulated_value": { "kind": "symbolic" } },
    { "name": "tax_region", "type": "TaxRegion", "simulated_value": { "kind": "symbolic" } },
    { "name": "validated_items", "type": "Vec<LineItem>", "simulated_value": { "kind": "computed", "expression": "validate_line_items(items)" } },
    { "name": "subtotal", "type": "Money", "simulated_value": { "kind": "computed", "expression": "sum(validated_items.map(|i| i.price * i.quantity))" } }
  ],
  "capabilities": ["TaxTable"],
  "errors_to_handle": ["TaxCalculationError"],
  "cost": {
    "budget_total": 5000,
    "cost_before_hole": 420,
    "budget_remaining": 4580
  },
  "candidates": [
    {
      "function": "lookup_tax_rate",
      "signature": "(TaxRegion, Money) -> Money ! [TaxCalculationError]",
      "match_quality": "exact",
      "requires_capabilities": ["TaxTable"],
      "estimated_cost": 100
    }
  ]
}
```

Agent fills:

```spore
-- Filling for ?compute_tax
lookup_tax_rate(tax_region, subtotal)
```

### Step 6: Compiler Confirms Completion

```
$ sporec check src/billing/invoice.spore

[ok] generate_invoice : (Customer, Vec<LineItem>, TaxRegion) -> Invoice ! [TaxCalculationError, InvalidLineItem]
  cost: 520 (within budget of 5000)
  all holes filled ✓
```

The function transitions from `partial` → `complete`. No downstream signature hashes change.

---

## 8. Edge Cases

### 8.1 Recursive Holes

A hole that references itself is meaningless—holes are not executable. The compiler detects this:

```spore
fn factorial(n: Int) -> Int ! []
where effects: pure, cost ≤ 1000
{
    if n <= 1 { 1 }
    else { n * ?factorial_step }
}
```

This is fine—`?factorial_step` is just a hole of type `Int`. It does **not** call itself. But if a developer writes:

```spore
fn bad(n: Int) -> Int ! []
where effects: pure
{
    ?self_ref + bad(n - 1)   -- bad is partial, but the recursive structure is fine
}
```

This is also valid. `bad` is partial, and `bad(n - 1)` is a call to a partial function, which makes the simulation stop at the recursive call too. The compiler reports:

```
[partial] bad: (Int) -> Int ! []
  holes: ?self_ref
  note: recursive call to partial function bad — simulation terminates at depth 1
```

### 8.2 Type Holes (Holes in Type Position)

Type holes are syntactically distinct from value holes: they use `?T_name` in **uppercase** position and represent unknown types.

```spore
fn convert(input: ?InputType) -> ?OutputType ! []
where
    effects: pure
{
    ?conversion_logic
}
```

**Type holes make the entire signature incomplete.** Unlike value holes (which are implementation-only), type holes affect the signature and therefore:
- The function **cannot** be exported.
- The function's signature hash is **undefined** (not computed).
- The compiler emits a `type-hole` diagnostic, distinct from value-hole.

Type holes are a design-time tool for sketching, not a production feature. They are reported separately:

```
$ sporec --holes --type-holes

Type holes:
  ?InputType   in convert (src/convert.spore:1)  — parameter type
  ?OutputType  in convert (src/convert.spore:1)  — return type

Value holes:
  ?conversion_logic  in convert (src/convert.spore:5)  — body
```

### 8.3 Conflicting Constraints

When a hole has contradictory constraints from its context, the compiler reports the conflict without failing:

```spore
fn conflicted(flag: Bool) -> Int ! []
where effects: pure
{
    let x: String = ?ambiguous   -- constraint 1: String (from let binding)
    let y: Int = x               -- constraint 2: Int (from assignment)
    y
}
```

```
[warning] hole ?ambiguous has conflicting type constraints:
  constraint 1: String (from let binding on line 4)
  constraint 2: Int    (from usage on line 5)
  note: no type satisfies both constraints simultaneously.
        The hole is reported with type `String` (from the nearest constraint).
        Filling this hole will likely require restructuring the surrounding code.
```

The hole still appears in `--holes` output and still gets a HoleReport. The Agent sees the conflict and can propose a restructuring.

### 8.4 Empty Function Body (Whole-Body Hole)

An empty function body is syntactic sugar for a single unnamed hole. The compiler auto-names it:

```spore
fn not_yet_implemented(x: Int, y: Int) -> Int ! [OverflowError]
where
    effects: pure, deterministic
    cost ≤ 100
{
}
```

Desugars to:

```spore
{
    ?not_yet_implemented_body
}
```

The auto-generated name is `{function_name}_body`. This allows `sporec --holes` to list it meaningfully.

```
$ sporec --holes

  ?not_yet_implemented_body
    in: not_yet_implemented (src/math.spore:1)
    type: Int ! [OverflowError]
    note: auto-generated (empty body)
```

### 8.5 Holes in Closures

Closures can contain holes. The hole captures the closure's bindings:

```spore
fn make_processor(config: Config) -> Fn(Data) -> Result ! [ProcessError]
where effects: pure
{
    |data: Data| -> Result ! [ProcessError] {
        ?process_with_config
    }
}
```

The HoleReport for `?process_with_config` includes both the closure parameter `data` and the captured `config`:

```json
{
  "bindings": [
    { "name": "data", "type": "Data", "simulated_value": { "kind": "symbolic", "origin": "closure parameter" } },
    { "name": "config", "type": "Config", "simulated_value": { "kind": "symbolic", "origin": "captured from make_processor" } }
  ]
}
```

### 8.6 Holes and `pure` Effect

A hole in a `pure` function is itself pure by definition (it does nothing). The compiler does not flag a purity violation for holes. However, the **filling** must be pure:

```spore
fn pure_fn(x: Int) -> Int ! []
where effects: pure
{
    ?must_be_pure   -- ok: hole is inert
}
```

If the Agent fills with an impure expression:

```
[error] pure_fn is declared pure but the filling of ?must_be_pure
        calls print() which has effect: io
```

---

## 9. Design Decisions & Rationale

### Why Named Holes Are Mandatory

Anonymous holes (`?`) might seem convenient, but they create two problems:
1. **Ambiguity in CLI queries**: `sporec --query-hole ?` is meaningless when there are multiple holes.
2. **Communication**: "Fill `?payment_logic`" is actionable. "Fill the hole on line 42" is fragile.

The cost of naming is low; the benefit to human-Agent communication is high.

### Why Holes Don't Affect Signature Hashes

Consider a team workflow: Alice writes a function signature and body with holes. Bob depends on Alice's function. If holes changed the hash, Bob's code would need recompilation every time Alice fills a hole—even though the *contract* (signature) never changed. By keeping holes out of the hash, the snapshot system remains stable during development.

### Why No Explicit Priority

No mainstream dependently-typed language with holes—Agda, Idris, Lean—includes a priority annotation on holes. The compiler's dependency analysis (§2.4) provides a better filling order than manual annotation because it reflects actual data flow: fill upstream holes first so downstream holes gain richer context (computed bindings instead of symbolic ones). When holes are independent, any order works equally well. The developer or Agent is always free to choose which hole to fill next; the suggested order is advisory, never enforced.

### Why `sporec` Outputs JSON, Not an Interactive UI

Agda's hole system is powerful but tightly coupled to Emacs. GHC's typed holes produce text diagnostics aimed at human readers. Spore's design targets a different consumer: **stateless Agents that parse structured output**. JSON is the natural format. Human-readable summaries are layered on top, never the primary output.

---

## 10. Summary

The Hole system turns "unfinished code" from a problem into a **structured collaboration protocol**:

```
Developer writes holes (intent, constraints)
         ↓
Compiler produces HoleReports (full context, candidates, budgets, dependencies)
         ↓
Agent reads HoleReports (zero extra context needed)
         ↓
Agent determines filling order (from dependency analysis)
         ↓
Agent proposes fillings (type-safe, cost-bounded, capability-checked)
         ↓
Compiler verifies (accepts or rejects with precise diagnostics)
         ↓
Repeat until complete
```

Every step is **compiler-mediated** and **CLI-accessible**. No IDE required. No statefulness required (each `sporec` invocation is self-contained). The hole is not a bug—it is a *typed, named, dependency-ordered, cost-bounded invitation to collaborate*.
