# Spore HoleReport v0.3 & Agent 工作流协议 — 设计文档

> **Status**: Draft
> **Version**: v0.3
> **Scope**: `sporec` 编译器 HoleReport 输出格式, `spore watch` 增量编译集成, Agent 状态机协议
> **Depends on**: Hole System v0.2, Cost system, Capability system, Error system, Incremental Compilation v0.1
> **Extends**: `archive/hole-system-v0.2.md` §3.2 HoleReport Structure

---

## 1. 概述

### 1.1 动机

HoleReport v0.2（见 `archive/hole-system-v0.2.md` §3.2）为每个 Hole 提供了名称、位置、期望类型、绑定、能力集、错误列表、成本预算和候选函数。这些信息足以让 Agent 做出基本的填充决策，但在以下场景中精度不足：

| 场景 | v0.2 的不足 | v0.3 的改进 |
|---|---|---|
| 多候选函数选择 | `match_quality: "exact" \| "partial"` 过于粗粒度 | 多维评分向量，量化每个维度的匹配程度 |
| 理解绑定之间的数据流 | 绑定列表是扁平的，无法看出 `validated` 依赖 `order` | 绑定依赖图，显式表达数据流关系 |
| 判断是否需要人工介入 | 无置信度信号 | 置信度与歧义度指标 |
| 处理多来源的错误 | `errors_to_handle` 是扁平列表 | 错误聚类，按来源操作分组并附带处理建议 |
| 多 Hole 并行填充 | 仅有 `hole.dependencies` 和 `dependent_holes` | Hole 依赖图，定义并行填充协议 |
| Agent 自动化工作流 | v0.2 描述了手动流程 | 完整的 Agent 状态机协议 |

### 1.2 扩展维度

本规范在 v0.2 HoleReport 基础上增加 **四个维度**：

1. **候选评分向量**（Candidate Scoring Vector）— 替代 `match_quality` 字符串
2. **绑定依赖图**（Binding Dependency Graph）— 表达绑定间数据流
3. **置信度与歧义度**（Confidence & Ambiguity）— 量化编译器对推荐的确定性
4. **错误聚类**（Error Clusters）— 按来源分组错误及处理建议

并定义 **Hole 依赖图与并行填充协议** 和 **Agent 工作流状态机**。

### 1.3 兼容性

- `schema` 字段从 `"spore/hole-report/v1"` 升级为 `"spore/hole-report/v2"`
- v0.3 是 v0.2 的**超集**：所有 v0.2 字段保留，新字段均为新增
- 不识别 v0.3 字段的工具可安全忽略它们

---

## 2. HoleReport v0.3 完整格式

以下 JSON Schema 展示所有字段。标注 `[v0.2]` 的为已有字段，标注 `[v0.3]` 的为本次新增。

```json
{
  "schema": "spore/hole-report/v2",                          // [v0.3] 升级版本号

  "hole": {                                                   // [v0.2]
    "name": "payment_logic",                                  // stable hole id; named holes reuse the source name, anonymous `?` holes get a compiler-generated id
    "location": {
      "file": "src/billing/charge.spore",
      "line": 42,
      "column": 5
    },
    "dependencies": ["validate_card"]
  },

  "type": {                                                   // [v0.2]
    "expected": "ChargeResult ! [PaymentFailed, GatewayTimeout]",
    "inferred_from": "return position of fn charge_customer"
  },

  "bindings": [                                               // [v0.2]
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

  "binding_dependencies": {                                   // [v0.3] 扩展 B
    "validated_card": ["customer"],
    "amount": []
  },

  "capabilities": ["PaymentGateway", "AuditLog"],             // [v0.2]

  "errors_to_handle": ["PaymentFailed", "GatewayTimeout"],    // [v0.2]

  "error_clusters": [                                         // [v0.3] 扩展 D
    {
      "source": "gateway_charge",
      "errors": ["PaymentFailed", "GatewayTimeout"],
      "handling_suggestion": "match on error type, retry GatewayTimeout"
    },
    {
      "source": "validate_card",
      "errors": ["InvalidCard"],
      "handling_suggestion": "early return with ?"
    }
  ],

  "cost": {                                                   // [v0.2]
    "budget_total": 2000,
    "cost_before_hole": 340,
    "budget_remaining": 1660
  },

  "candidates": [                                             // [v0.2] + [v0.3] 扩展 A
    {
      "function": "gateway_charge",
      "signature": "(Card, Money) -> ChargeResult ! [PaymentFailed, GatewayTimeout]",
      "requires_capabilities": ["PaymentGateway"],
      "estimated_cost": 800,
      "scores": {                                             // [v0.3] 替代 match_quality
        "type_match": 0.95,
        "cost_fit": 0.80,
        "capability_fit": 1.0,
        "error_coverage": 0.90
      },
      "overall": 0.91,
      "adjustments": ["需要类型转换: Option<Card> → Card"]
    },
    {
      "function": "retry_charge",
      "signature": "(Card, Money, RetryPolicy) -> ChargeResult ! [PaymentFailed, GatewayTimeout]",
      "requires_capabilities": ["PaymentGateway"],
      "estimated_cost": 1500,
      "scores": {
        "type_match": 0.70,
        "cost_fit": 0.10,
        "capability_fit": 1.0,
        "error_coverage": 0.90
      },
      "overall": 0.62,
      "adjustments": ["需要额外参数: RetryPolicy", "成本接近预算上限"]
    }
  ],

  "confidence": {                                             // [v0.3] 扩展 C
    "type_inference": "certain",
    "candidate_ranking": "unique_best",
    "ambiguous_count": 0,
    "recommendation": "gateway_charge is the best match with 0.91 overall score"
  },

  "dependent_holes": ["finalize_charge"],                     // [v0.2]

  "enclosing_function": {                                     // [v0.2]
    "name": "charge_customer",
    "signature": "(Customer, Money) -> ChargeResult ! [PaymentFailed, GatewayTimeout]",
    "effects": ["deterministic"],
    "full_cost_budget": 2000
  }
}
```

下文中的 `hole.name` 都表示 **稳定 hole id**，不要求一定是源代码里的显式名称。
本协议只覆盖**函数体中的可填充 expression hole**；函数签名中的 `?` 用于类型推断，不单独生成 HoleReport 条目。

---

## 3. 扩展 A：候选评分向量

### 3.1 动机

v0.2 的 `match_quality: "exact" | "partial (needs RetryPolicy)"` 是人类可读的字符串，但 Agent 无法基于它进行数值比较和排序。v0.3 引入四维评分向量，每个维度量化一个关键匹配指标。

### 3.2 评分维度定义

| 维度 | 符号 | 范围 | 含义 |
|---|---|---|---|
| **类型匹配度** | `type_match` | `[0, 1]` | 候选函数的返回类型与参数类型对 Hole 期望类型的匹配程度 |
| **成本适配度** | `cost_fit` | `[0, 1]` | 候选函数的预估成本与剩余预算的适配比 |
| **能力适配度** | `capability_fit` | `{0, 1}` | 候选函数所需能力是否全部可用（布尔量） |
| **错误覆盖度** | `error_coverage` | `[0, 1]` | 候选函数声明的错误类型中，已被 Hole 上下文处理/声明的比例 |

### 3.3 评分公式

**类型匹配度 `type_match`**：

```
type_match(candidate, hole) =
    let return_score = type_similarity(candidate.return_type, hole.expected_type)
    let param_scores = candidate.params.map(p =>
        max(bindings.map(b => type_similarity(b.type, p.type)))
    )
    let param_score = if param_scores.is_empty() then 1.0
                      else mean(param_scores)
    0.6 × return_score + 0.4 × param_score
```

其中 `type_similarity(A, B)` 定义为：

| 关系 | 得分 |
|---|---|
| `A = B`（精确相等） | 1.0 |
| `A <: B` 或 `B <: A`（子类型关系） | 0.9 |
| 存在已知转换 `A → B` | 0.7 |
| 类型构造器相同，参数部分匹配 | 0.5 |
| 无关类型 | 0.0 |

**成本适配度 `cost_fit`**：

```
cost_fit(candidate, hole) =
    if candidate.estimated_cost ≤ hole.cost.budget_remaining then
        1.0 - (candidate.estimated_cost / hole.cost.budget_remaining) × 0.3
    else
        max(0, 1.0 - (candidate.estimated_cost - hole.cost.budget_remaining)
                      / hole.cost.budget_remaining)
```

直觉：成本远低于预算 → 接近 1.0；恰好等于预算 → 0.7；超出预算 → 快速衰减至 0。

**能力适配度 `capability_fit`**：

```
capability_fit(candidate, hole) =
    if candidate.requires_capabilities ⊆ hole.capabilities then 1.0
    else 0.0
```

布尔量：要么完全满足（1.0），要么不满足（0.0）。不满足时 Agent 不应选择该候选。

**错误覆盖度 `error_coverage`**：

```
error_coverage(candidate, hole) =
    let declared = candidate.declared_errors
    let handled  = hole.errors_to_handle ∪ hole.enclosing_function.error_list
    |declared ∩ handled| / |declared|
```

如果 `|declared| = 0`，则 `error_coverage = 1.0`。

### 3.4 综合评分与排序

**综合评分 `overall`**：

```
overall = w₁ × type_match + w₂ × cost_fit + w₃ × capability_fit + w₄ × error_coverage
```

默认权重：

| 权重 | 值 | 理由 |
|---|---|---|
| w₁ (`type_match`) | 0.40 | 类型安全是首要约束 |
| w₂ (`cost_fit`) | 0.20 | 成本重要但非决定性 |
| w₃ (`capability_fit`) | 0.25 | 能力不满足则完全不可用 |
| w₄ (`error_coverage`) | 0.15 | 错误处理影响正确性但可补全 |

权重在编译器中硬编码，不可配置。理由：可配置权重增加认知负担，且不同项目间的最优权重差异不大。未来如果有明确需求，可通过 `spore.toml` 开放配置。

**排序算法**：

1. 按 `overall` 降序排列
2. `overall` 相同时，按 `type_match` 降序（类型安全优先）
3. 仍然相同时，按 `cost_fit` 降序（倾向低成本方案）
4. 仍然相同时，按候选函数名称字典序（确保稳定排序）

### 3.5 adjustments 字段

`adjustments` 是人类可读的注释数组，由编译器生成，描述候选函数与 Hole 之间需要的适配操作：

- 类型转换提示：`"需要类型转换: Option<Card> → Card"`
- 缺少参数提示：`"需要额外参数: RetryPolicy"`
- 成本警告：`"成本接近预算上限"`
- 能力缺失警告：`"缺少能力: NetworkAccess"`

Agent 应将 `adjustments` 作为辅助信息，结合 `scores` 做出决策。

---

## 4. 扩展 B：绑定依赖图

### 4.1 动机

v0.2 的 `bindings` 列表是扁平结构——Agent 看到 `customer`、`amount`、`validated_card` 三个绑定，但不知道 `validated_card` 依赖于 `customer`。这意味着 Agent 无法理解数据流方向，可能在生成代码时使用尚未初始化的中间值。

### 4.2 格式定义

```json
"binding_dependencies": {
  "<binding_name>": ["<dependency_1>", "<dependency_2>", ...],
  ...
}
```

**语义**：邻接表形式，key 为绑定名称，value 为该绑定**直接依赖**的其他绑定名称列表。

**规则**：

| 规则 | 说明 |
|---|---|
| 空数组 `[]` | 该绑定不依赖任何其他绑定（独立绑定，通常是函数参数） |
| 键集合 | 必须与 `bindings` 数组中的 `name` 集合一致 |
| 无自环 | 绑定不可依赖自身 |
| 无循环 | 依赖图必须是 DAG（有向无环图） |

### 4.3 依赖提取算法

编译器从 SSA（Static Single Assignment）形式的数据流分析中提取依赖关系：

```
对于 Hole 处每个可见绑定 b：
    binding_dependencies[b.name] = {
        d.name | d ∈ bindings,
                 b 的定义表达式中引用了 d
    }
```

**示例**：

```spore
fn charge_customer(customer: Customer, amount: Money) -> ChargeResult ! [PaymentFailed]
    uses [PaymentGateway]
    cost [2000, 0, 0, 0]
{
    let validated_card = validate_card(customer.card)  -- 引用 customer
    let order = create_order(customer, amount)          -- 引用 customer, amount
    let validated = verify_order(order)                 -- 引用 order
    let charged = process_charge(validated, validated_card)  -- 引用 validated, validated_card
    ?finalize
}
```

提取结果：

```json
"binding_dependencies": {
  "customer": [],
  "amount": [],
  "validated_card": ["customer"],
  "order": ["customer", "amount"],
  "validated": ["order"],
  "charged": ["validated", "validated_card"]
}
```

**可视化**（Agent 可据此理解数据流方向）：

```
customer ─────┬──→ validated_card ──→ charged
              │                        ↑
              ├──→ order ──→ validated ─┘
              │
amount ───────┘
```

### 4.4 Agent 使用方式

Agent 应优先使用依赖图中"末端"绑定（入度高、出度低），因为它们携带了最多的已计算信息。对于 Hole 填充代码，Agent 应确保只引用 Hole 处已可用的绑定（即依赖链已完全求值的绑定）。

---

## 5. 扩展 C：置信度与歧义度

### 5.1 动机

即使有评分向量，Agent 仍需知道编译器对推荐结果的**确定程度**。例如：当最佳候选的 `overall` 为 0.91 而第二名为 0.89 时，Agent 应该意识到选择是有歧义的。

### 5.2 格式定义

```json
"confidence": {
  "type_inference": "certain" | "partial" | "unknown",
  "candidate_ranking": "unique_best" | "ambiguous" | "no_candidates",
  "ambiguous_count": 3,
  "recommendation": "gateway_charge is the best match with 0.91 overall score"
}
```

### 5.3 字段语义

**`type_inference`** — 编译器对 Hole 期望类型的推断置信度：

| 值 | 条件 | 含义 |
|---|---|---|
| `"certain"` | 类型完全确定（显式标注或上下文唯一约束） | Agent 可信赖 `type.expected` |
| `"partial"` | 类型部分确定（已知类型构造器，参数未知） | Agent 需推断类型参数 |
| `"unknown"` | 无法推断类型（Hole 处于无约束上下文） | Agent 需自行决定类型 |

**`candidate_ranking`** — 候选排名的确定性：

| 值 | 条件 | 含义 |
|---|---|---|
| `"unique_best"` | 最佳候选的 `overall` 与第二名差距 ≥ 0.15 | Agent 可直接选择最佳候选 |
| `"ambiguous"` | 最佳候选与第二名差距 < 0.15，或有多个 `overall` 相同的候选 | Agent 需进一步分析或请求人工确认 |
| `"no_candidates"` | 候选列表为空 | Agent 需从零构造填充代码 |

**`ambiguous_count`** — 当 `candidate_ranking = "ambiguous"` 时，`overall` 在最佳候选 ±0.15 范围内的候选数量。其他情况下为 `0`。

**`recommendation`** — 编译器生成的人类可读推荐文本。格式不固定，仅供辅助参考。

### 5.4 置信度判定流程

```
输入: type_info, candidates[]

-- 类型推断置信度
if type_info.expected 完全确定 (无类型变量):
    type_inference = "certain"
elif type_info.expected 包含部分类型变量:
    type_inference = "partial"
else:
    type_inference = "unknown"

-- 候选排名置信度
if candidates.is_empty():
    candidate_ranking = "no_candidates"
    ambiguous_count = 0
elif candidates.len() == 1:
    candidate_ranking = "unique_best"
    ambiguous_count = 0
else:
    let gap = candidates[0].overall - candidates[1].overall
    if gap >= 0.15:
        candidate_ranking = "unique_best"
        ambiguous_count = 0
    else:
        candidate_ranking = "ambiguous"
        ambiguous_count = candidates.filter(c =>
            candidates[0].overall - c.overall < 0.15
        ).len()
```

---

## 6. 扩展 D：错误聚类

### 6.1 动机

v0.2 的 `errors_to_handle: ["PaymentFailed", "GatewayTimeout"]` 是扁平列表。Agent 不知道这些错误分别由哪个操作产生，也不知道最佳处理策略。v0.3 按来源操作对错误进行聚类，并附带处理建议。

### 6.2 格式定义

```json
"error_clusters": [
  {
    "source": "<function_name>",
    "errors": ["<Error1>", "<Error2>"],
    "handling_suggestion": "<human-readable suggestion>"
  }
]
```

### 6.3 字段语义

| 字段 | 类型 | 说明 |
|---|---|---|
| `source` | `string` | 产生错误的候选函数名称，或 `"context"` 表示来自 Hole 上下文约束 |
| `errors` | `string[]` | 该来源可能产生的错误类型列表 |
| `handling_suggestion` | `string` | 编译器建议的处理方式（人类可读） |

### 6.4 聚类算法

```
对于 Hole 的每个候选函数 c：
    declared_errors = c.signature 中声明的错误类型
    if declared_errors 非空:
        创建 cluster:
            source = c.function
            errors = declared_errors
            handling_suggestion = generate_suggestion(declared_errors, hole.context)

对于 Hole 上下文中未被任何候选覆盖的错误类型：
    创建 cluster:
        source = "context"
        errors = [未覆盖的错误类型]
        handling_suggestion = "需由填充代码显式处理"
```

**处理建议生成规则**：

| 错误特征 | 建议 |
|---|---|
| 单一错误类型，可传播 | `"early return with ?"` |
| 多个错误类型，同一来源 | `"match on error type"` |
| 可重试的错误（含 `Timeout`、`Retry` 等关键词） | `"retry with backoff"` |
| 错误类型在封闭函数的 `!` 列表中 | `"propagate to caller"` |

### 6.5 示例

```spore
fn process_payment(card: Card, amount: Money) -> Receipt ! [PaymentFailed, GatewayTimeout, InvalidCard]
    uses [PaymentGateway]
    cost [3000, 0, 0, 0]
{
    let valid = validate_card(card)
    ?charge_and_receipt
}
```

```json
"error_clusters": [
  {
    "source": "gateway_charge",
    "errors": ["PaymentFailed", "GatewayTimeout"],
    "handling_suggestion": "match on error type, retry GatewayTimeout with backoff"
  },
  {
    "source": "validate_card",
    "errors": ["InvalidCard"],
    "handling_suggestion": "early return with ?"
  }
]
```

---

## 7. Hole 依赖图与并行填充

### 7.1 动机

当项目中存在多个 Hole 时，它们之间存在数据流依赖。v0.2 在每个 HoleReport 中提供了 `hole.dependencies` 和 `dependent_holes`，但缺乏**全局视角**。v0.3 定义项目级 Hole 依赖图，支持多 Agent 并行填充。

### 7.2 Hole 依赖图格式

`spore watch --json` 在启动时和每次编译后输出：

```json
{
  "type": "hole_graph_update",
  "hole_graph": {
    "total": 8,
    "filled": 3,
    "ready_to_fill": ["validate_input", "check_auth"],
    "blocked": ["process_order", "send_receipt"],
    "dependency_edges": [
      ["validate_input", "process_order"],
      ["check_auth", "process_order"],
      ["process_order", "send_receipt"]
    ]
  }
}
```

### 7.3 字段语义

| 字段 | 类型 | 说明 |
|---|---|---|
| `total` | `int` | 项目中 Hole 总数（含已填充） |
| `filled` | `int` | 已成功填充的 Hole 数量 |
| `ready_to_fill` | `string[]` | 所有依赖已满足、可立即填充的 Hole id |
| `blocked` | `string[]` | 存在未满足依赖的 Hole id |
| `dependency_edges` | `[string, string][]` | 有向边列表，元素均为稳定 Hole id；`[A, B]` 表示 B 依赖 A（A 应先于 B 填充） |

### 7.4 图构建算法

```
输入: 所有未填充 Hole 的 HoleReport 集合 H

dependency_edges = []
对于每个 hole ∈ H:
    对于每个 dep ∈ hole.dependencies:
        if dep ∈ H:  -- dep 也是未填充的 Hole
            dependency_edges.append([dep, hole.name])

ready_to_fill = {
    hole.name | hole ∈ H,
                hole.name 在 dependency_edges 中不作为任何边的目标出现,
                hole 尚未被填充
}

blocked = { hole.name | hole ∈ H, hole.name ∉ ready_to_fill, hole 尚未被填充 }
```

**等价定义**：`ready_to_fill` = 剩余未填充子图中入度为 0 的节点集合。

### 7.5 并行填充协议

**核心规则**：`ready_to_fill` 中的所有 Hole 可被多个 Agent **同时填充**，互不干扰。

```
最大并行度 = |ready_to_fill| = 剩余 DAG 的反链宽度（antichain width）
```

**并行填充流程**：

```
1. Agent 从 ready_to_fill 中选取一个 Hole
2. 获取该 Hole 的 HoleReport v0.3
3. 生成填充代码并写入文件
4. 编译器检测文件变化 → 增量编译
5. 编译通过 → Hole 标记为 filled → 重新计算 ready_to_fill
6. 新的 ready_to_fill 可能包含之前 blocked 的 Hole
```

**冲突处理**：如果两个 Agent 同时尝试填充同一个 Hole：

- 先写入文件的 Agent **获胜**（file lock 机制）
- 后写入的 Agent 收到 `CONFLICT` 信号
- 收到 `CONFLICT` 的 Agent 应放弃当前 Hole，从 `ready_to_fill` 中选取另一个

### 7.6 环检测

Hole 依赖图必须是 DAG。如果检测到环，编译器报告编译错误：

```
[error] circular hole dependency detected:
  ?compute_total → ?apply_discount → ?compute_total

  Holes in a cycle cannot be filled in any order.
  Break the cycle by restructuring the code.
```

### 7.7 复杂度分析

| 操作 | 复杂度 | 说明 |
|---|---|---|
| 图构建 | O(\|V\| + \|E\|) | V = Hole 数量, E = 依赖边数量 |
| 环检测 | O(\|V\| + \|E\|) | 拓扑排序的副产品 |
| ready_to_fill 计算 | O(\|V\|) | 遍历入度数组 |
| 填充后更新 | O(\|E_out\|) | 仅更新被填充 Hole 的出边目标的入度 |

所有操作均在图规模上线性，适用于大型项目。

---

## 8. Agent 工作流状态机

### 8.1 状态定义

```
                    ┌────────────────────────────────────────────────┐
                    │                                                │
                    ▼                                                │
              ┌──────────┐                                           │
              │ DISCOVER │─── spore watch --json 启动                │
              └────┬─────┘    接收 Hole 列表 + 依赖图                │
                   │                                                 │
                   │ 选取 ready_to_fill 中的 Hole                    │
                   ▼                                                 │
              ┌──────────┐                                           │
              │ ANALYZE  │─── 获取 HoleReport v0.3                   │
              └────┬─────┘    分析评分、依赖、置信度                 │
                   │                                                 │
                   │ 生成填充代码                                    │
                   ▼                                                 │
              ┌──────────┐                                           │
              │ PROPOSE  │─── 写入填充代码到源文件                   │
              └────┬─────┘                                           │
                   │                                                 │
                   │ 文件变化触发增量编译                            │
                   ▼                                                 │
              ┌──────────┐                                           │
              │ VERIFY   │─── 等待编译结果                           │
              └────┬─────┘                                           │
                   │                                                 │
            ┌──────┴──────┐                                          │
            │             │                                          │
            ▼             ▼                                          │
      ┌──────────┐  ┌──────────┐                                     │
      │ ACCEPT   │  │ REJECT   │                                     │
      └────┬─────┘  └────┬─────┘                                     │
           │              │                                          │
           │              │ Agent 自主决定:                           │
           │              │ 重试 / 换方案 / 停止                     │
           │              └──────────────────────────────────────────┘
           │
           │ 更新 Hole 状态, 重新计算 ready_to_fill
           │
           │ 还有未填充的 Hole?
           ├── 是 ──→ 返回 DISCOVER
           └── 否 ──→ COMMIT (所有 Hole 已填充)
```

### 8.2 状态详细说明

#### DISCOVER

- **触发**: `spore watch --json` 启动，或前一个 ACCEPT 完成后
- **输入**: NDJSON 事件流
- **输出**: Hole 列表、依赖图 (`hole_graph_update` 事件)
- **Agent 行为**: 解析 `ready_to_fill`，选择要处理的 Hole

#### ANALYZE

- **触发**: Agent 选中一个 `ready_to_fill` 中的 Hole
- **输入**: `sporec --query-hole <hole-id> --json`
- **输出**: 完整的 HoleReport v0.3
- **Agent 行为**:
  - 检查 `confidence.candidate_ranking`
  - 如果 `"unique_best"` → 直接使用最佳候选
  - 如果 `"ambiguous"` → 分析 `scores` 各维度，结合 `adjustments` 做决策
  - 如果 `"no_candidates"` → 基于 `type`、`bindings`、`binding_dependencies` 自行构造代码
  - 参考 `error_clusters` 确定错误处理策略

#### PROPOSE

- **触发**: Agent 完成代码生成
- **行为**: 将填充代码写入源文件，替换被选中的 hole（命名或匿名）
- **约束**: 一次只修改一个 Hole（原子操作）

#### VERIFY

- **触发**: 文件系统变化被 `spore watch` 检测到
- **行为**: 增量编译自动触发
- **输出**: `compile_result` 事件

#### ACCEPT

- **触发**: 编译通过（类型检查、成本检查、能力检查均通过）
- **行为**:
  - 更新 Hole 状态为 `filled`
  - 重新计算依赖图 → 发送 `hole_graph_update` 事件
  - 之前 `blocked` 的 Hole 可能进入 `ready_to_fill`
- **Agent 行为**: 返回 DISCOVER 处理下一个 Hole

#### REJECT

- **触发**: 编译失败
- **输出**: 失败诊断信息（见 §8.3）
- **Agent 行为**: 自主决策，无需人工介入
  - 读取诊断信息，理解失败原因
  - 选择策略：重试（修改代码）、换候选函数、停止并报告

### 8.3 失败诊断格式

编译失败时，`compile_result` 事件包含结构化诊断信息：

```json
{
  "type": "compile_result",
  "status": "rejected",
  "hole": "validate_input",
  "attempt": {
    "code": "validate_items(raw_input)",
    "timestamp": "2025-01-15T10:23:45Z"
  },
  "diagnostics": {
    "errors": [
      {
        "code": "E0301",
        "message": "type mismatch: expected Vec<ValidItem>, found Vec<RawItem>",
        "location": {
          "file": "src/orders.spore",
          "line": 18,
          "column": 5
        },
        "suggestion": "consider using validate_items(raw_input).map(|i| i.into())"
      }
    ],
    "root_cause": "type_mismatch",
    "fix_hints": [
      "尝试使用 Money.from_int() 进行类型转换",
      "候选函数 validate_items 的返回类型为 Vec<RawItem>，而非期望的 Vec<ValidItem>"
    ]
  }
}
```

**诊断字段说明**：

| 字段 | 说明 |
|---|---|
| `errors` | 编译器报告的所有错误，每个包含错误码、消息、位置和建议 |
| `root_cause` | 编译器判断的根本原因类别（`type_mismatch` / `cost_exceeded` / `capability_violation` / `error_unhandled`） |
| `fix_hints` | 编译器生成的修复建议列表（中文/英文混合，人类可读） |

### 8.4 设计决策说明

**无 RETRY 状态**：REJECT 后 Agent 直接回到 PROPOSE（或 ANALYZE，如果需要重新选择候选）。编译器在 REJECT 时返回完整诊断信息，Agent 据此自主决策。显式 RETRY 状态会增加状态机复杂度而不增加信息量。

**无 ESCALATE 状态**：Agent 是完全自主的。如果 Agent 无法填充某个 Hole（例如多次 REJECT），它应：

1. 记录所有尝试和诊断信息
2. 将该 Hole 标记为 `agent_skipped`
3. 继续处理其他 `ready_to_fill` 中的 Hole
4. 最终报告中列出所有 `agent_skipped` 的 Hole 及原因

### 8.5 并发 Agent 协议

多个 Agent 可同时运行，每个处理不同的 `ready_to_fill` Hole：

```
Agent-1: DISCOVER → ANALYZE(?validate_input) → PROPOSE → VERIFY → ACCEPT
Agent-2: DISCOVER → ANALYZE(?check_auth)     → PROPOSE → VERIFY → ACCEPT
                                                         (并行)
```

**冲突处理**：

| 场景 | 处理方式 |
|---|---|
| 两个 Agent 填充不同 Hole | 正常并行，互不干扰 |
| 两个 Agent 填充同一 Hole | 先写入者获胜，后者收到 `CONFLICT` |
| Agent 填充导致另一 Hole 的 HoleReport 失效 | `hole_graph_update` 事件通知，Agent 重新 ANALYZE |

**CONFLICT 信号格式**：

```json
{
  "type": "compile_result",
  "status": "conflict",
  "hole": "validate_input",
  "message": "hole already filled by another agent",
  "filled_by": {
    "timestamp": "2025-01-15T10:23:44Z"
  }
}
```

---

## 9. 与增量编译的集成

### 9.1 NDJSON 事件流

`spore watch --json` 输出 NDJSON（Newline-Delimited JSON）事件流，每行一个 JSON 对象：

```
$ spore watch --json
{"type":"hole_graph_update","hole_graph":{...}}
{"type":"hole_update","hole":"validate_input","report":{...}}
{"type":"compile_result","status":"accepted","hole":"validate_input"}
{"type":"hole_graph_update","hole_graph":{...}}
...
```

### 9.2 事件类型

| 事件类型 | 触发时机 | 包含数据 |
|---|---|---|
| `hole_graph_update` | 启动时；每次编译完成后 | 完整的 `hole_graph` 对象（§7.2） |
| `hole_update` | Hole 的 HoleReport 因编译而更新时 | 稳定 Hole id + 完整 HoleReport v0.3 |
| `compile_result` | 增量编译完成时 | 编译状态 + 诊断信息（§8.3） |

### 9.3 事件流时序

```
文件变化
    │
    ▼
spore watch 检测到变化
    │
    ▼
增量编译启动
    │
    ├──→ compile_result (accepted / rejected / conflict)
    │
    ▼
重新分析 Hole 状态
    │
    ├──→ hole_update (如果 HoleReport 有变化)
    │
    ▼
重新计算依赖图
    │
    └──→ hole_graph_update
```

### 9.4 Agent 消费事件流

Agent 应以流式方式读取 NDJSON，按事件类型分发处理：

```
fn consume_events(stdin) -> ():
    match read_line(stdin):
        Some(line) =>
            event = parse_json(line)
            match event.type:
                "hole_graph_update" => update_local_graph(event.hole_graph)
                "hole_update"       => update_hole_report(event.hole, event.report)
                "compile_result"    => handle_compile_result(event)
            consume_events(stdin)   -- 递归处理下一行
        None => ()                  -- stdin 关闭，结束
```

---

## 10. 完整示例

以下示例完整展示一个 Agent 会话，从启动到所有 Hole 填充完成。

### 10.1 初始状态

项目中有 5 个 Hole：

```spore
-- src/orders.spore
fn process_order(order: Order) -> Receipt ! [ValidationError, PaymentFailed, OutOfStock]
    uses [Inventory, PaymentGateway, EmailService]
    cost [10000, 0, 0, 0]
{
    let valid = ?validate_input
    let reserved = ?reserve_stock
    let payment = ?process_payment
    let receipt = create_receipt(valid, reserved, payment)
    ?send_receipt
    receipt
}

-- src/auth.spore
fn check_auth(token: Token) -> User ! [Unauthorized]
    uses [AuthService]
    cost [500, 0, 0, 0]
{
    ?check_auth
}
```

### 10.2 Step 1: DISCOVER — 接收 Hole 列表和依赖图

Agent 启动 `spore watch --json`，收到第一个事件：

```json
{
  "type": "hole_graph_update",
  "hole_graph": {
    "total": 5,
    "filled": 0,
    "ready_to_fill": ["validate_input", "check_auth"],
    "blocked": ["reserve_stock", "process_payment", "send_receipt"],
    "dependency_edges": [
      ["validate_input", "reserve_stock"],
      ["validate_input", "process_payment"],
      ["reserve_stock", "send_receipt"],
      ["process_payment", "send_receipt"]
    ]
  }
}
```

依赖图可视化：

```
validate_input ──┬──→ reserve_stock ────┬──→ send_receipt
                 │                      │
                 └──→ process_payment ──┘

check_auth (独立)
```

**Agent 决策**: `ready_to_fill` 中有 2 个 Hole，可并行处理。Agent-1 选择 `validate_input`，Agent-2 选择 `check_auth`。

### 10.3 Step 2: ANALYZE — 获取 HoleReport

Agent-1 获取 `?validate_input` 的 HoleReport：

```json
{
  "schema": "spore/hole-report/v2",
  "hole": {
    "name": "validate_input",
    "location": { "file": "src/orders.spore", "line": 9, "column": 5 },
    "dependencies": []
  },
  "type": {
    "expected": "ValidOrder ! [ValidationError]",
    "inferred_from": "used as argument to reserve_stock and process_payment"
  },
  "bindings": [
    { "name": "order", "type": "Order", "simulated_value": { "kind": "symbolic", "origin": "parameter" } }
  ],
  "binding_dependencies": {
    "order": []
  },
  "capabilities": ["Inventory", "PaymentGateway", "EmailService"],
  "errors_to_handle": ["ValidationError"],
  "error_clusters": [
    {
      "source": "validate_order",
      "errors": ["ValidationError"],
      "handling_suggestion": "propagate to caller"
    }
  ],
  "cost": {
    "budget_total": 10000,
    "cost_before_hole": 0,
    "budget_remaining": 10000
  },
  "candidates": [
    {
      "function": "validate_order",
      "signature": "(Order) -> ValidOrder ! [ValidationError]",
      "requires_capabilities": [],
      "estimated_cost": 200,
      "scores": {
        "type_match": 1.0,
        "cost_fit": 0.99,
        "capability_fit": 1.0,
        "error_coverage": 1.0
      },
      "overall": 1.0,
      "adjustments": []
    }
  ],
  "confidence": {
    "type_inference": "certain",
    "candidate_ranking": "unique_best",
    "ambiguous_count": 0,
    "recommendation": "validate_order is a perfect match"
  },
  "dependent_holes": ["reserve_stock", "process_payment"],
  "enclosing_function": {
    "name": "process_order",
    "signature": "(Order) -> Receipt ! [ValidationError, PaymentFailed, OutOfStock]",
    "effects": ["deterministic"],
    "full_cost_budget": 10000
  }
}
```

**Agent 分析**：`confidence.candidate_ranking = "unique_best"`，`overall = 1.0`，直接使用 `validate_order(order)`。

### 10.4 Step 3: PROPOSE — 写入填充代码

Agent-1 将 `?validate_input` 替换为 `validate_order(order)`。

同时，Agent-2 将 `?check_auth` 替换为 `verify_token(token)`。

### 10.5 Step 4: VERIFY → ACCEPT — 编译通过

文件变化触发增量编译。Agent 收到事件：

```json
{"type":"compile_result","status":"accepted","hole":"validate_input"}
{"type":"compile_result","status":"accepted","hole":"check_auth"}
{"type":"hole_graph_update","hole_graph":{"total":5,"filled":2,"ready_to_fill":["reserve_stock","process_payment"],"blocked":["send_receipt"],"dependency_edges":[["reserve_stock","send_receipt"],["process_payment","send_receipt"]]}}
```

**现在 `ready_to_fill` 变为 `["reserve_stock", "process_payment"]`** — 两个新 Hole 解锁。

### 10.6 Step 5: PROPOSE → REJECT — 编译失败

Agent-1 尝试填充 `?reserve_stock`，写入 `reserve_items(valid.items, warehouse_id)`。

编译失败，收到：

```json
{
  "type": "compile_result",
  "status": "rejected",
  "hole": "reserve_stock",
  "attempt": {
    "code": "reserve_items(valid.items, warehouse_id)",
    "timestamp": "2025-01-15T10:24:12Z"
  },
  "diagnostics": {
    "errors": [
      {
        "code": "E0201",
        "message": "undefined binding: warehouse_id",
        "location": { "file": "src/orders.spore", "line": 10, "column": 35 },
        "suggestion": "did you mean to use a field of `valid`?"
      }
    ],
    "root_cause": "undefined_binding",
    "fix_hints": [
      "warehouse_id 不在当前作用域中",
      "可用的绑定: order, valid",
      "尝试: reserve_items(valid.items, valid.warehouse_id)"
    ]
  }
}
```

### 10.7 Step 6: Agent 读取诊断，重新 PROPOSE

Agent 读取 `fix_hints`，理解到 `warehouse_id` 应从 `valid` 中获取。重新生成代码：

```spore
reserve_items(valid.items, valid.warehouse_id)
```

写入文件，触发增量编译。

```json
{"type":"compile_result","status":"accepted","hole":"reserve_stock"}
```

编译通过。

### 10.8 Step 7: ACCEPT → COMMIT — 所有 Hole 填充完成

Agent 继续填充 `process_payment` 和 `send_receipt`，均编译通过。

最终状态：

```json
{
  "type": "hole_graph_update",
  "hole_graph": {
    "total": 5,
    "filled": 5,
    "ready_to_fill": [],
    "blocked": [],
    "dependency_edges": []
  }
}
```

**所有 Hole 已填充 → COMMIT。** Agent 可执行 `git commit` 或通知开发者。

### 10.9 完整时序图

```
时间轴    Agent-1                    Agent-2                    编译器
─────────────────────────────────────────────────────────────────────────────
t0        DISCOVER                   DISCOVER                   hole_graph_update (5 holes)
          ├ ready: [validate_input,  ├ ready: [validate_input,
          │         check_auth]      │         check_auth]
          │                          │
t1        ANALYZE(validate_input)    ANALYZE(check_auth)        (并行)
          │                          │
t2        PROPOSE: validate_order    PROPOSE: verify_token      (并行写入)
          │(order)                   │(token)
          │                          │
t3        ·                          ·                          增量编译
          │                          │
t4        ACCEPT ✓                   ACCEPT ✓                   hole_graph_update (3 remaining)
          │                          │                          ready: [reserve_stock,
          │                          │                                  process_payment]
          │                          │
t5        ANALYZE(reserve_stock)     ANALYZE(process_payment)   (并行)
          │                          │
t6        PROPOSE: reserve_items     PROPOSE: charge_card       (并行写入)
          │(valid.items,             │(valid.payment_info,
          │ warehouse_id) ← 错误!    │ valid.amount)
          │                          │
t7        REJECT ✗                   ACCEPT ✓                   compile_result
          │ diagnostics:             │
          │ "undefined: warehouse_id"│
          │                          │
t8        PROPOSE: reserve_items     ·                          增量编译
          │(valid.items,             │
          │ valid.warehouse_id)      │
          │                          │
t9        ACCEPT ✓                   ·                          hole_graph_update (1 remaining)
          │                                                     ready: [send_receipt]
          │
t10       ANALYZE(send_receipt)                                 (Agent-1 继续)
          │
t11       PROPOSE: send_email                                   增量编译
          │(receipt, valid.email)
          │
t12       ACCEPT ✓                                              hole_graph_update (0 remaining)
          │
t13       COMMIT ── 所有 Hole 已填充 ──────────────────────────  ✓ 完成
```

---

## 附录 A：HoleReport v0.2 → v0.3 字段变更摘要

| 变更类型 | 字段 | 说明 |
|---|---|---|
| **升级** | `schema` | `"spore/hole-report/v1"` → `"spore/hole-report/v2"` |
| **替代** | `candidates[].match_quality` | 被 `candidates[].scores` + `candidates[].overall` + `candidates[].adjustments` 替代 |
| **新增** | `candidates[].scores` | 四维评分向量（§3） |
| **新增** | `candidates[].overall` | 加权综合评分（§3.4） |
| **新增** | `candidates[].adjustments` | 适配注释列表（§3.5） |
| **新增** | `binding_dependencies` | 绑定依赖图（§4） |
| **新增** | `confidence` | 置信度与歧义度（§5） |
| **新增** | `error_clusters` | 错误聚类（§6） |
| **保留** | 所有其他 v0.2 字段 | 语义不变 |
