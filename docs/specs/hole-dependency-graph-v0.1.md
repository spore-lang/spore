# Hole 依赖图 — 设计文档 v0.1

> **Status**: Draft
> **Scope**: `sporec` 编译器, `spore` Codebase Manager, Agent 并行工作流
> **Depends on**: Hole System v0.2, Cost Model v0.1, Type System v0.1

---

## 1. 概述

### 1.1 动机

Spore 的 Hole 系统允许一个代码库中同时存在多个未填充的 hole（`?name`）。当 AI Agent 需要批量填充这些 hole 时，一个关键问题出现了：**哪些 hole 可以并行填充，哪些必须按顺序填充？**

如果 hole B 的类型推断依赖于 hole A 的输出类型，那么 B 必须在 A 填充之后才能被填充。反之，如果两个 hole 之间没有依赖关系，它们可以被不同的 Agent 同时填充。

### 1.2 核心洞察

> **Hole 依赖关系构成有向无环图（DAG）。并行填充的最大吞吐量等于该 DAG 最宽反链的宽度。**

本文档形式化描述该依赖图的构建、排序、并行调度与增量更新算法。

---

## 2. 定义

### 2.1 Hole 依赖图

**定义 2.1（Hole 依赖图）.** 给定一个模块 M，其 Hole 依赖图 G = (V, E) 定义如下：

- **V** = M 中所有未填充 hole 的集合
- **E** = { (h₁, h₂) | h₂ 的类型推断或约束求解依赖于 h₁ 的输出 }

每条边 (h₁, h₂) ∈ E 表示"h₁ 必须先于 h₂ 被填充"。

### 2.2 依赖类型

边 (h₁, h₂) 的依赖关系分为三类：

| 依赖类型 | 记号 | 含义 |
|----------|------|------|
| **类型依赖** | `type` | h₂ 的期望类型包含一个类型变量，该变量仅在 h₁ 填充后才能被求解 |
| **值依赖** | `value` | h₂ 的可用绑定中包含一个值，该值的数据流源自 h₁ 的输出 |
| **代价依赖** | `cost` | h₂ 的代价预算依赖于 h₁ 的实际代价（从父函数预算中扣除） |

**定义 2.2（类型依赖）.** 若 h₂ 的期望类型 τ₂ 中存在类型变量 α，且 α 的唯一约束来源是 h₁ 的输出类型，则 (h₁, h₂) 为类型依赖。

**定义 2.3（值依赖）.** 若 h₂ 的可用绑定集合 Γ₂ 中存在绑定 b，且在 SSA 形式下 b 的数据流可追溯至 h₁ 的输出值，则 (h₁, h₂) 为值依赖。

**定义 2.4（代价依赖）.** 若 h₁ 和 h₂ 在同一函数体的同一顺序块中，且 h₁ 的位置先于 h₂，则 h₂ 的剩余代价预算依赖于 h₁ 的实际代价。此时 (h₁, h₂) 为代价依赖。

### 2.3 辅助定义

**定义 2.5（就绪集合）.** ready(G) = { h ∈ V | in-degree(h) = 0 }，即没有任何前驱的 hole 集合。

**定义 2.6（填充层）.** 将 G 进行拓扑分层，第 k 层 Lₖ 定义为：从 G 中移除 L₀, L₁, ..., Lₖ₋₁ 后的就绪集合。

**定义 2.7（最大并行度）.** parallelism(G) = max(|Lₖ|)，即所有层中最大的层宽。

---

## 3. 图构建算法

### 3.1 主算法

```pseudocode
// NOTE: This is algorithm pseudocode; Spore itself has no loop constructs —
//       use recursion + higher-order functions (each/fold/map/filter).

function build_hole_graph(module: TypedAST) -> Graph:
    holes = find_all_holes(module)

    edges = holes |> flat_map(fn(h) {
        // 值依赖：追踪绑定的数据流来源
        value_edges = h.available_bindings |> filter_map(fn(b) {
            source = trace_data_source(b)
            if source is HoleOutput(h'):
                Some(Edge(from=h', to=h, kind="value"))
            else: None
        })

        // 类型依赖：追踪类型变量的约束来源
        type_edges = free_type_vars(h.expected_type) |> filter_map(fn(tv) {
            source = trace_type_source(tv)
            if source is HoleOutput(h'):
                Some(Edge(from=h', to=h, kind="type"))
            else: None
        })

        // 代价依赖：同一顺序块中的先序 hole
        cost_edges = if h.cost_budget depends on another hole h':
            [Edge(from=h', to=h, kind="cost")]
        else: []

        value_edges ++ type_edges ++ cost_edges
    })

    // 去重：同一对 (h', h) 可能存在多种依赖，保留所有类型标注
    return Graph(vertices=holes, edges=deduplicate(edges))
```

### 3.2 trace_data_source

沿 SSA 形式的数据流反向追踪一个绑定的来源：

```pseudocode
// NOTE: This is algorithm pseudocode; Spore itself has no loop constructs.

function trace_data_source(binding: Binding) -> Source:
    match binding.origin:
        Parameter(p)       => return ParameterSource(p)
        LetBinding(expr)   => return trace_expr_source(expr)
        HoleOutput(h)      => return HoleOutput(h)
        FunctionCall(f, args) =>
            // 如果 f 的任何参数来自 hole，则传递依赖
            args |> find_map(fn(arg) {
                src = trace_data_source(arg)
                if src is HoleOutput(h): Some(HoleOutput(h))
                else: None
            }) |> unwrap_or(ConcreteSource(f))
```

### 3.3 trace_type_source

沿类型推断约束链反向追踪一个类型变量的来源：

```pseudocode
// NOTE: This is algorithm pseudocode; Spore itself has no loop constructs.

function trace_type_source(tv: TypeVar) -> Source:
    constraints = get_constraints_for(tv)
    constraints |> find_map(fn(c) {
        match c:
            UnifyWith(HoleOutputType(h)) => Some(HoleOutput(h))
            UnifyWith(ConcreteType(t))   => Some(ConcreteSource(t))
            UnifyWith(OtherTypeVar(tv')) => Some(trace_type_source(tv'))
            _                            => None
    }) |> unwrap_or(Unconstrained)
```

### 3.4 示例：图构建过程

给定以下代码：

```spore
fn process_order(order: RawOrder) -> Receipt ! ValidationError | PaymentError
    uses [PaymentGateway, Inventory]
    cost [5000, 0, 0, 0]
{
    let validated = ?validate_order              // h1
    let stock_ok  = ?check_inventory             // h2
    let charged   = ?charge_payment              // h3, 使用 validated 和 stock_ok
    let receipt   = ?generate_receipt             // h4, 使用 charged
    receipt
}
```

分析：
- `?charge_payment`（h3）的绑定中包含 `validated`（源自 h1）和 `stock_ok`（源自 h2）→ 值依赖
- `?generate_receipt`（h4）的绑定中包含 `charged`（源自 h3）→ 值依赖
- h3 的剩余代价预算依赖于 h1、h2 的实际代价 → 代价依赖
- h4 的剩余代价预算依赖于 h1、h2、h3 的实际代价 → 代价依赖

构建的图：

```
    h1 (validate_order)     h2 (check_inventory)
         \                    /
      value\              /value
           v            v
         h3 (charge_payment)
               |
           value|
               v
         h4 (generate_receipt)
```

---

## 4. 拓扑排序与填充顺序

### 4.1 分层拓扑排序

```pseudocode
// NOTE: This is algorithm pseudocode; Spore itself has no loop constructs.

function compute_fill_order(G: Graph) -> Result[List[Set[Hole]], CycleError]:
    // 第一步：环检测
    if has_cycle(G):
        cycle_path = find_cycle(G)
        return Err(CycleError(cycle_path))

    // 第二步：分层拓扑排序（Kahn 算法变体）
    //   使用递归 + fold 代替 while 循环
    layers = []
    remaining = copy(V)
    in_degree = compute_in_degrees(G)

    fn build_layers(remaining, in_degree, layers):
        if remaining is empty: return layers
        ready = remaining |> filter(fn(h) { in_degree[h] == 0 })
        assert ready is not empty   // 无环保证此断言成立
        new_in_degree = ready |> fold(in_degree, fn(deg, h) {
            successors(h) |> fold(deg, fn(d, s) { d[s] -= 1; d })
        })
        build_layers(remaining - ready, new_in_degree, layers ++ [ready])

    return Ok(build_layers(remaining, in_degree, []))
```

### 4.2 语义

- `layers[0]` = 无依赖的 hole → 最先填充（可并行）
- `layers[1]` = 依赖全部在 `layers[0]` 中的 hole → 在第 0 层完成后填充（可并行）
- `layers[k]` = 依赖全部在 `layers[0..k-1]` 中的 hole
- ...以此类推，直到所有 hole 被填充

### 4.3 层内排序：影响力启发式

在同一层内，hole 按**传递依赖者数量**降序排列（与 Hole System v0.2 §2.4 一致）：

```pseudocode
// NOTE: This is algorithm pseudocode; Spore itself has no loop constructs.

function rank_within_layer(layer: Set[Hole], G: Graph) -> List[Hole]:
    scores = layer |> map(fn(h) { (h, count_transitive_dependents(h, G)) })
    scores |> sort_descending_by(fn((_, s)) { s }) |> map(fn((h, _)) { h })
```

影响力越高的 hole 越优先分配给 Agent，因为填充它能解锁更多下游工作。

---

## 5. 并行填充策略

### 5.1 最大并行度

**定理 5.1.** 对于 Hole 依赖图 G，最优并行填充所需的最少 Agent 数量等于：

```
parallelism(G) = max { |Lₖ| : k = 0, 1, ..., depth(G) }
```

这等于 G 作为偏序集的**最宽反链宽度**（Dilworth 定理的推论）。

### 5.2 调度协议

```
时刻 0: 计算 ready_set = layers[0] = {h1, h2}
        Agent A1 ← h1, Agent A2 ← h2
        A1、A2 并行填充

时刻 1: h1、h2 均通过编译验证 ✓
        增量更新图 → 新 ready_set = {h3, h4, h5}
        A1 ← h3, A2 ← h4, A3 ← h5
        A1、A2、A3 并行填充

时刻 2: h3、h4、h5 通过验证 ✓
        增量更新图 → 新 ready_set = {h6}
        A1 ← h6
        A1 填充

时刻 3: 所有 hole 已填充 → COMMIT
```

### 5.3 Agent 分配策略

```pseudocode
// NOTE: This is algorithm pseudocode; Spore itself has no loop constructs.

function assign_agents(ready: Set[Hole], agents: List[Agent]) -> Assignment:
    ranked = rank_within_layer(ready, G)

    ranked |> enumerate() |> fold({}, fn(assignment, (i, hole)) {
        agent = agents[i % len(agents)]   // 轮询分配
        assignment[hole] = agent
        assignment
    })
```

当 Agent 数量少于就绪 hole 数量时，采用轮询分配。影响力高的 hole 优先被分配给空闲 Agent。

### 5.4 冲突解决

两个 Agent 不得同时填充同一 hole。通过文件级锁实现互斥：

```pseudocode
function try_fill(agent: Agent, hole: Hole) -> FillResult:
    if not acquire_lock(hole.file, agent.id):
        return FillResult {
            status: "conflict",
            hole: hole.name,
            filled_by: lock_holder(hole.file)
        }

    result = agent.generate_fill(hole)
    verify = sporec_check(hole.file)

    if verify.ok:
        commit_fill(hole, result)
        release_lock(hole.file, agent.id)
        return FillResult { status: "success", hole: hole.name }
    else:
        rollback_fill(hole)
        release_lock(hole.file, agent.id)
        return FillResult { status: "verify_failed", errors: verify.errors }
```

冲突时返回的消息格式：

```json
{
  "status": "conflict",
  "hole": "validate_order",
  "filled_by": "agent-1"
}
```

---

## 6. 环检测与错误报告

### 6.1 环的含义

循环 hole 依赖是**编译错误**。如果 hole A 的类型推断需要 hole B 的输出，同时 hole B 又需要 hole A，则两者都不可能被填充。

### 6.2 检测算法

采用标准 DFS 着色法，时间复杂度 O(|V| + |E|)：

```pseudocode
// NOTE: This is algorithm pseudocode; Spore itself has no loop constructs.

function detect_cycle(G: Graph) -> Option[List[Hole]]:
    color = V |> fold({}, fn(m, h) { m.insert(h, WHITE) })
    parent = {}

    function dfs(h):
        color[h] = GRAY
        successors(h) |> find_map(fn(s) {
            if color[s] == GRAY:
                // 发现环：从 s 回溯到 s
                return Some(extract_cycle(parent, s, h))
            if color[s] == WHITE:
                parent[s] = h
                dfs(s)
            else: None
        })
        color[h] = BLACK
        return None

    V |> filter(fn(h) { color[h] == WHITE }) |> find_map(fn(h) { dfs(h) })
```

### 6.3 错误输出格式

```
error[H0301]: circular hole dependency detected
  --> src/order.spore
  |
  | ?validate_order depends on ?check_inventory  (value dependency, line 12)
  | ?check_inventory depends on ?validate_order  (type dependency, line 18)
  |
  = help: break the cycle by providing a concrete type annotation on one hole
    e.g., ?validate_order : ValidatedOrder
```

### 6.4 修复建议

编译器在检测到环时提供修复建议：

1. **添加类型注解**：在环中的某个 hole 上添加显式类型注解，切断类型依赖
2. **拆分函数**：将环中的 hole 移到不同函数中，通过函数签名提供类型信息
3. **引入中间绑定**：添加一个具体类型的 let 绑定，切断值依赖链

---

## 7. 增量更新

### 7.1 动机

当一个 hole 被填充后，无需重建整个图。仅需局部更新。

### 7.2 更新算法

```pseudocode
// NOTE: This is algorithm pseudocode; Spore itself has no loop constructs.

function on_hole_filled(G: Graph, filled: Hole, fill_result: FillResult) -> GraphUpdate:
    // 第一步：移除已填充的 hole
    V = V - {filled}

    // 第二步：移除所有关联边
    removed_edges = { e ∈ E | e.from == filled or e.to == filled }
    E = E - removed_edges

    // 第三步：更新后继 hole 的入度
    newly_ready = removed_edges
        |> filter(fn((_, successor)) { successor ∈ V })
        |> filter(fn((_, successor)) { in_degree(successor) == 0 })
        |> map(fn((_, successor)) { successor })
        |> to_set()

    // 第四步：检查是否有新 hole 被揭示（填充可能暴露新代码路径）
    new_holes = find_revealed_holes(fill_result.new_code)
    if new_holes is not empty:
        V = V ∪ new_holes
        new_edges = compute_edges_for(new_holes, G)
        E = E ∪ new_edges

    // 第五步：发出更新事件
    emit(HoleGraphUpdate {
        filled: filled,
        newly_ready: newly_ready,
        new_holes: new_holes,
        remaining: |V|
    })

    return GraphUpdate { newly_ready, new_holes }
```

### 7.3 复杂度

单次增量更新的时间复杂度为 **O(|neighbors(filled)|)**，即被填充 hole 的邻居数量。不需要对整个图进行重新计算。

### 7.4 Watch 模式集成

在 `sporec --watch` 模式下，每次文件变更后自动触发增量更新：

```
$ sporec --watch --holes

[watch] file changed: src/order.spore
[update] hole ?validate_order filled
[graph]  removed 2 edges, 0 new holes revealed
[ready]  newly ready: ?charge_payment, ?check_inventory
[status] 3/6 holes remaining, max parallelism: 2
```

---

## 8. 完全性证明

**定理 8.1（完全性）.** 若 Hole 依赖图 G = (V, E) 是 DAG，则 `compute_fill_order` 算法发现并输出所有可填充的 hole。

**证明.** 对层数 k 进行归纳。

**基础情况（k = 0）：**
L₀ = { h ∈ V | in-degree(h) = 0 }。这些 hole 没有任何前驱依赖，其类型、绑定和代价预算均已完全确定。因此它们是可填充的。由于我们取了所有入度为 0 的节点，不存在遗漏。

**归纳步骤（k → k+1）：**
假设 L₀, L₁, ..., Lₖ 已被正确计算且所有 hole 均已被填充。设 G' 为从 G 中移除 L₀ ∪ L₁ ∪ ... ∪ Lₖ 后的子图。

Lₖ₊₁ = { h ∈ V(G') | in-degree_{G'}(h) = 0 }

对于任意 h ∈ Lₖ₊₁：h 在原图 G 中的所有前驱均属于 L₀ ∪ ... ∪ Lₖ，已被填充。因此 h 的类型约束和值绑定均已求解，h 是可填充的。

反之，若存在可填充的 hole h ∉ L₀ ∪ ... ∪ Lₖ₊₁，则 h 在 G' 中入度 > 0，即存在未填充的前驱。这与"h 可填充"矛盾。

因此 Lₖ₊₁ 恰好包含所有在第 k+1 轮可填充的 hole。 ∎

**推论 8.2.** 若 G 是 DAG，则算法在 depth(G) + 1 轮后终止，且填充所有 hole。

**推论 8.3.** 数据流分析的穷尽性保证：由于 `trace_data_source` 和 `trace_type_source` 遍历了所有 SSA 边和类型约束链，不存在未被发现的依赖关系。

---

## 9. 复杂度分析

| 操作 | 时间复杂度 | 说明 |
|------|-----------|------|
| 图构建 | O(\|V\| × B) | B = 每个 hole 的平均绑定数 |
| 环检测 | O(\|V\| + \|E\|) | DFS 着色法 |
| 拓扑排序（分层） | O(\|V\| + \|E\|) | Kahn 算法变体 |
| 层内排序 | O(\|V\| × \|V\|) | 传递依赖者计数（可缓存优化至 O(\|V\| + \|E\|)） |
| 单次增量更新 | O(\|neighbors\|) | 仅涉及被填充 hole 的邻居 |
| 完整填充会话 | O(\|V\|² + \|E\|) | 最坏情况，所有增量更新之和 |

**空间复杂度**：O(|V| + |E|)，用于存储图结构和入度表。

---

## 10. JSON 输出格式

### 10.1 hole_graph_update 事件

在 `sporec --watch --json` 模式下，每次图状态变更时输出：

```json
{
  "type": "hole_graph_update",
  "timestamp": "2026-03-30T12:00:00Z",
  "graph": {
    "total_holes": 8,
    "filled_holes": 3,
    "remaining_holes": 5,
    "ready_to_fill": [
      { "name": "validate_input", "file": "src/order.spore", "line": 42 },
      { "name": "check_auth", "file": "src/auth.spore", "line": 15 }
    ],
    "blocked": [
      { "name": "process_order", "blocked_by": ["validate_input", "check_auth"] },
      { "name": "send_receipt", "blocked_by": ["process_order"] },
      { "name": "update_ledger", "blocked_by": ["process_order"] }
    ],
    "edges": [
      { "from": "validate_input", "to": "process_order", "type": "value" },
      { "from": "check_auth", "to": "process_order", "type": "value" },
      { "from": "process_order", "to": "send_receipt", "type": "type" },
      { "from": "process_order", "to": "update_ledger", "type": "value" }
    ],
    "max_parallelism": 2,
    "estimated_layers": 3
  }
}
```

### 10.2 字段说明

| 字段 | 类型 | 说明 |
|------|------|------|
| `type` | string | 事件类型，固定为 `"hole_graph_update"` |
| `timestamp` | string | ISO 8601 时间戳 |
| `graph.total_holes` | int | 项目中 hole 总数（含已填充） |
| `graph.filled_holes` | int | 已填充的 hole 数量 |
| `graph.remaining_holes` | int | 剩余未填充的 hole 数量 |
| `graph.ready_to_fill` | array | 当前就绪可填充的 hole 列表（入度为 0） |
| `graph.blocked` | array | 被阻塞的 hole 及其阻塞源 |
| `graph.edges` | array | 依赖边列表，含类型标注 |
| `graph.max_parallelism` | int | 当前就绪层的最大并行度 |
| `graph.estimated_layers` | int | 剩余图的预估层数 |

### 10.3 CLI 输出

非 JSON 模式下的人类可读输出：

```
$ spore holes --graph

Hole Dependency Graph (6 holes, 5 edges):

  Layer 0 (parallel):
    ● ?validate_input    src/order.spore:42   type: ValidatedInput
    ● ?check_auth        src/auth.spore:15    type: AuthResult

  Layer 1 (parallel, after layer 0):
    ○ ?process_order     src/order.spore:58   type: OrderResult
      ← depends on: ?validate_input (value), ?check_auth (value)

  Layer 2 (parallel, after layer 1):
    ○ ?send_receipt      src/notify.spore:23  type: Receipt
      ← depends on: ?process_order (type)
    ○ ?update_ledger     src/ledger.spore:31  type: LedgerEntry
      ← depends on: ?process_order (value)

  Layer 3 (after layer 2):
    ○ ?finalize          src/order.spore:72   type: FinalStatus
      ← depends on: ?send_receipt (value), ?update_ledger (value)

  Max parallelism: 2
  Estimated fill rounds: 4
```

---

## 11. 完整示例

### 11.1 场景描述

一个订单处理系统包含 6 个 hole：

```spore
fn handle_order(raw: RawOrder) -> FinalStatus ! ValidationErr | PaymentErr | NotifyErr
    uses [PaymentGateway, Inventory, EmailService]
    cost [10000, 0, 0, 0]
{
    let valid   = ?validate_input                          // h1
    let authed  = ?check_auth                              // h2
    let result  = ?process_order                           // h3, 使用 valid 和 authed
    let receipt = ?send_receipt                             // h4, 使用 result
    let ledger  = ?update_ledger                           // h5, 使用 result
    ?finalize                                              // h6, 使用 receipt 和 ledger
}
```

### 11.2 依赖图（ASCII）

```
    h1 (?validate_input)      h2 (?check_auth)
        \                       /
     value\                 /value
          v               v
        h3 (?process_order)
          /               \
     type/                 \value
        v                   v
    h4 (?send_receipt)   h5 (?update_ledger)
         \                 /
      value\           /value
            v         v
        h6 (?finalize)
```

### 11.3 分层结果

```
compute_fill_order(G) = [
    Layer 0: { h1, h2 },      // 无依赖，并行填充
    Layer 1: { h3 },           // 依赖 h1, h2
    Layer 2: { h4, h5 },      // 依赖 h3，可并行
    Layer 3: { h6 },           // 依赖 h4, h5
]
```

最大并行度 = max(2, 1, 2, 1) = **2**

### 11.4 填充过程

**第 0 轮：**
```
Agent A1 → ?validate_input (h1)
Agent A2 → ?check_auth (h2)
[并行执行]
```

A1 将 h1 填充为：
```spore
validate_schema(raw) |> check_business_rules
```

A2 将 h2 填充为：
```spore
auth_service.verify(raw.token) |> require_role(Admin)
```

编译验证通过 ✓。增量更新图：移除 h1、h2，h3 变为就绪。

**第 1 轮：**
```
Agent A1 → ?process_order (h3)
[单个 Agent 执行]
```

A1 将 h3 填充为：
```spore
let reserved = inventory.reserve(valid.items)
gateway.charge(authed.user, valid.total, reserved)
```

编译验证通过 ✓。增量更新图：移除 h3，h4 和 h5 变为就绪。

**第 2 轮：**
```
Agent A1 → ?send_receipt (h4)
Agent A2 → ?update_ledger (h5)
[并行执行]
```

A1 将 h4 填充为：
```spore
email_service.send(authed.user.email, Receipt.from(result))
```

A2 将 h5 填充为：
```spore
ledger.append(LedgerEntry.from(result, timestamp.now()))
```

编译验证通过 ✓。增量更新图：移除 h4、h5，h6 变为就绪。

**第 3 轮：**
```
Agent A1 → ?finalize (h6)
[单个 Agent 执行]
```

A1 将 h6 填充为：
```spore
FinalStatus.completed(receipt, ledger)
```

编译验证通过 ✓。所有 hole 已填充。

```
$ sporec check src/order_pipeline.spore

[ok] handle_order : (RawOrder) -> FinalStatus ! ValidationErr | PaymentErr | NotifyErr
  cost: 7340 (within budget of 10000)
  all holes filled ✓

COMMIT
```

### 11.5 JSON 输出序列

初始状态：

```json
{
  "type": "hole_graph_update",
  "timestamp": "2026-03-30T12:00:00Z",
  "graph": {
    "total_holes": 6,
    "filled_holes": 0,
    "remaining_holes": 6,
    "ready_to_fill": [
      { "name": "validate_input", "file": "src/order_pipeline.spore", "line": 8 },
      { "name": "check_auth", "file": "src/order_pipeline.spore", "line": 9 }
    ],
    "blocked": [
      { "name": "process_order", "blocked_by": ["validate_input", "check_auth"] },
      { "name": "send_receipt", "blocked_by": ["process_order"] },
      { "name": "update_ledger", "blocked_by": ["process_order"] },
      { "name": "finalize", "blocked_by": ["send_receipt", "update_ledger"] }
    ],
    "edges": [
      { "from": "validate_input", "to": "process_order", "type": "value" },
      { "from": "check_auth", "to": "process_order", "type": "value" },
      { "from": "process_order", "to": "send_receipt", "type": "type" },
      { "from": "process_order", "to": "update_ledger", "type": "value" },
      { "from": "send_receipt", "to": "finalize", "type": "value" },
      { "from": "update_ledger", "to": "finalize", "type": "value" }
    ],
    "max_parallelism": 2,
    "estimated_layers": 4
  }
}
```

第 0 轮填充后：

```json
{
  "type": "hole_graph_update",
  "timestamp": "2026-03-30T12:01:15Z",
  "graph": {
    "total_holes": 6,
    "filled_holes": 2,
    "remaining_holes": 4,
    "ready_to_fill": [
      { "name": "process_order", "file": "src/order_pipeline.spore", "line": 10 }
    ],
    "blocked": [
      { "name": "send_receipt", "blocked_by": ["process_order"] },
      { "name": "update_ledger", "blocked_by": ["process_order"] },
      { "name": "finalize", "blocked_by": ["send_receipt", "update_ledger"] }
    ],
    "edges": [
      { "from": "process_order", "to": "send_receipt", "type": "type" },
      { "from": "process_order", "to": "update_ledger", "type": "value" },
      { "from": "send_receipt", "to": "finalize", "type": "value" },
      { "from": "update_ledger", "to": "finalize", "type": "value" }
    ],
    "max_parallelism": 2,
    "estimated_layers": 3
  }
}
```

---

## 附录 A：与 Hole System v0.2 的关系

本文档形式化了 Hole System v0.2 §2.4（填充顺序）中描述的依赖分析。具体对应关系：

| Hole System v0.2 | 本文档 |
|---|---|
| §2.4 填充顺序 | §4 拓扑排序与填充顺序 |
| §3.3 `hole.dependencies` 字段 | §3 图构建算法 |
| §3.3 `dependent_holes` 字段 | §7 增量更新（newly_ready） |
| `spore holes --suggest-order` | §10.3 CLI 输出 |

## 附录 B：未来扩展

1. **跨模块依赖**：当前图构建限于单模块。未来版本将支持跨模块 hole 依赖，利用模块签名中的 `partial` 标记传播依赖信息。
2. **动态优先级调整**：根据 Agent 的历史填充速度和 hole 的估计复杂度，动态调整层内优先级。
3. **部分环解决**：探索在环中自动插入类型注解以打破循环依赖的可能性。
4. **可视化**：在 `spore` CLI 中集成 DAG 可视化工具，输出 DOT 格式供 Graphviz 渲染。
