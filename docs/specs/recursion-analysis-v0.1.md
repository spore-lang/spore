# 递归代价分析 — 三层组合方案 v0.1

> **Status**: Draft
> **Scope**: 编译器代价分析、类型检查、Hole 系统
> **Depends on**: cost-model-v0.1, signature-syntax-v0.2, compiler-pipeline-v0.1
> **日期**: 2025-07

---

## 一、概述

Spore 语言**没有循环结构**（`for`/`while`/`loop` 均已从语言中移除）。
所有迭代行为通过**递归 + 高阶函数**（`map`/`fold`/`filter` 等）表达。
这意味着递归代价分析不是可选的优化——它是代价模型的**核心支柱**。

如果编译器无法分析递归代价，则绝大多数程序的代价都是 `unbounded`，
四维代价系统（`compute(op)`, `alloc(cell)`, `io(call)`, `parallel(lane)`）将形同虚设。

### 设计目标

| 目标 | 描述 |
|------|------|
| 高覆盖率 | ~90% 的实际递归代码可自动或半自动获得代价上界 |
| 零负担 | 简单情况无需开发者手动标注 |
| 可逃逸 | 无法分析的代码不会阻塞编译，只产生警告 |
| 可组合 | 递归代价与高阶函数代价公式无缝组合 |

### 三层方案

```
          覆盖率
  ┌────────────────────────────────────────────┐
  │  Tier 1: 结构递归自动检测        ~70%     │  ← 编译器全自动
  ├────────────────────────────────────────────┤
  │  Tier 2: 声明式验证              ~20%     │  ← 开发者写 cost ≤ expr
  ├────────────────────────────────────────────┤
  │  Tier 3: @unbounded 逃逸         ~10%     │  ← 显式放弃代价约束
  └────────────────────────────────────────────┘
```

设计原则：**能自动推导的绝不要求手动标注，无法推导的绝不阻塞编译。**

---

## 二、Tier 1: 结构递归自动检测（Structural Recursion Auto-Detection）

### 2.1 定义

一个函数是**结构递归**的，当且仅当：

1. 它至少有一个参数在**每条递归调用路径上严格递减**
2. 递减关系基于**良基关系**（well-founded relation）

形式化定义：

```
设 f(x₁, ..., xₙ) 为递归函数，
若 ∃ i ∈ {1, ..., n}, ∀ 递归调用 f(y₁, ..., yₙ):
    yᵢ ≺ xᵢ  （其中 ≺ 是类型 Tᵢ 上的良基关系）
则 f 是结构递归的。
```

### 2.2 良基递减模式

编译器识别以下递减模式：

| 模式 | 源参数 → 递归参数 | 良基关系 | 典型代价 |
|------|-------------------|---------|---------|
| 自然数递减 | `n → n - 1`（需 `n > 0` 守卫） | `<` on ℕ | O(n) |
| 列表尾部 | `list → list.tail` | 子列表关系 | O(n) |
| 树子节点（单侧） | `tree → tree.left` 或 `tree → tree.right` | 子树关系 | O(log n) 平衡 / O(n) 最坏 |
| 树子节点（双侧） | `tree → tree.left` 且 `tree → tree.right` | 子树关系 | O(n) |
| 枚举变体解构 | `match x { Variant(inner) => f(inner) }` | 结构子项关系 | O(depth) |
| 元组投影 | `(a, b) → a` 或 `(a, b) → b`（严格更小） | 结构子项关系 | 取决于投影目标 |
| 整数等分 | `n → n / 2`（需 `n > 0` 守卫） | `<` on ℕ | O(log n) |

### 2.3 代价推导规则

结构递归一旦被识别，编译器自动推导代价上界：

**线性递归（单次递归调用）：**
```
f(n) 中 n 每次递减 1:
    cost(f, n) = n × cost_per_step
    → O(n)

f(list) 中 list = list.tail:
    cost(f, list) = len(list) × cost_per_step
    → O(n)

f(n) 中 n = n / 2:
    cost(f, n) = log(n) × cost_per_step
    → O(log n)
```

**二叉递归（两次递归调用）：**
```
f(tree) 中分别递归 tree.left 和 tree.right:
    cost(f, tree) = nodes(tree) × cost_per_step
    → O(n)

f(n) 中递归 f(n-1) 和 f(n-2):
    cost(f, n) = 2^n × cost_per_step（指数级！）
    → 编译器发出警告，建议改用尾递归或添加 memo
```

其中 `cost_per_step` 是函数体中除递归调用外的所有操作的四维代价，
按照 cost-model-v0.1 中的原语代价表计算。

### 2.4 检测算法

```
algorithm detect_structural_recursion(f):
    1. 提取 f 中所有递归调用点 {call₁, call₂, ..., callₖ}
    2. 对每个 callⱼ:
        a. 识别哪些参数发生了变化: changed_args(callⱼ)
        b. 对每个变化的参数 xᵢ:
            检查 yᵢ ≺ xᵢ 是否成立（通过模式匹配语法结构）
        c. 如果存在至少一个参数在所有路径上严格递减:
            mark callⱼ as structurally_decreasing
    3. 如果 ALL 递归调用都是 structurally_decreasing:
        → f 是结构递归的
        → 根据递减模式推导代价上界
    4. 否则:
        → f 不是结构递归的
        → 进入 Tier 2 或 Tier 3
```

检测复杂度：O(|call_graph|)，在类型检查阶段完成。

### 2.5 示例

**自然数递减——阶乘：**
```spore
fn factorial(n: Int) -> Int {
    match n {
        0 => 1,
        n => n * factorial(n - 1),
    }
}
```

编译器输出：
```
✓ structural recursion detected: n decreases by 1 on each call
  cost = n × (1[*] + 3[call]) = n × 4
  → O(n)
```

**列表尾部——求和：**
```spore
fn sum(list: List<Int>) -> Int {
    match list {
        [] => 0,
        [head, ..tail] => head + sum(tail),
    }
}
```

编译器输出：
```
✓ structural recursion detected: list decreases to tail (sublist)
  cost = len(list) × (1[+] + 3[call]) = len(list) × 4
  → O(n)
```

**树遍历——单侧递归：**
```spore
fn depth<T>(tree: Tree<T>) -> Int {
    match tree {
        Leaf(_) => 0,
        Node(left, _, right) => 1 + max(depth(left), depth(right)),
    }
}
```

编译器输出：
```
✓ structural recursion detected: tree decreases to left/right (subtree, binary)
  cost = nodes(tree) × (1[+] + 1[max] + 3[call] × 2) = nodes(tree) × 9
  → O(n)
```

---

## 三、Tier 2: 声明式验证（Declarative Verification）

### 3.1 动机

部分递归函数虽然可证明终止且代价有界，但递减关系不属于 Tier 1 能自动识别的模式。
此时由开发者提供代价上界声明，编译器负责验证。

### 3.2 语法

在函数签名中通过 `cost ≤ expr` 子句声明代价上界：

```spore
fn ackermann(m: Int, n: Int) -> Int
    cost ≤ ackermann_bound(m, n)
{
    match (m, n) {
        (0, n) => n + 1,
        (m, 0) => ackermann(m - 1, 1),
        (m, n) => ackermann(m - 1, ackermann(m, n - 1)),
    }
}
```

### 3.3 `decreases` 子句（可选）

当终止性证明需要显式的递减度量时，开发者可提供 `decreases` 子句：

```spore
fn gcd(a: Int, b: Int) -> Int
    decreases a + b
    cost ≤ log(max(a, b))
{
    match b {
        0 => a,
        _ => gcd(b, a % b),
    }
}
```

`decreases expr` 语义：编译器验证 `expr` 在每次递归调用时严格递减且非负。

### 3.4 验证方法

编译器按以下顺序尝试验证 `cost ≤ expr`：

```
验证策略优先级:
  1. 调用树归纳（Induction on Call Tree）
     若 cost(recursive_call) < cost(current_call)，
     且 base case 代价有界 → 上界成立

  2. 单调性分析（Monotonicity Analysis）
     若上界表达式关于递减参数单调递减，
     且 base case 满足 → 上界成立

  3. 简单算术验证（Arithmetic Verification）
     将递归展开 k 步，代入参数值，
     验证 cost(展开) ≤ expr(原始参数)
```

形式化：

```
设 f(x) 的声明代价为 B(x)，递归调用参数为 x'，
验证目标：∀x. cost_body(x) + cost(f(x')) ≤ B(x)

其中:
  cost_body(x) = 函数体中除递归调用外的代价
  cost(f(x')) ≤ B(x')（归纳假设）

故需验证：cost_body(x) + B(x') ≤ B(x)
```

### 3.5 验证失败处理

如果编译器无法验证 `cost ≤ expr`：

```
WARNING [unverified-cost-bound] gcd 的代价上界无法自动验证。
  声明: cost ≤ log(max(a, b))
  原因: 编译器无法证明 log(max(b, a % b)) < log(max(a, b))

  选项:
  (a) 提供 decreases 子句帮助编译器推导
  (b) 标记为 @unbounded
  (c) 如果你确信上界正确，添加 @trust_cost 抑制此警告
```

注意：验证失败产生 **warning**（不是 error），程序仍可编译。
这确保了系统的渐进式采用——开发者可以先标注代价，后续完善证明。

### 3.6 示例

**归并排序——声明式验证：**
```spore
fn merge_sort<T>(list: List<T>) -> List<T>
    where T: Ord
    decreases len(list)
    cost ≤ len(list) * log(len(list))
{
    match list {
        [] => [],
        [x] => [x],
        _ => {
            (left, right) = split_at(list, len(list) / 2)
            merge(merge_sort(left), merge_sort(right))
        },
    }
}
```

编译器验证过程：
```
1. decreases len(list):
   ✓ len(left) < len(list)  (split_at 保证)
   ✓ len(right) < len(list) (split_at 保证)

2. cost ≤ n × log(n) where n = len(list):
   cost_body = cost(split_at) + cost(merge) = O(n)
   cost(merge_sort(left)) ≤ (n/2) × log(n/2)  (归纳假设)
   cost(merge_sort(right)) ≤ (n/2) × log(n/2) (归纳假设)
   total ≤ n + 2 × (n/2) × log(n/2) = n + n × log(n/2) = n + n × (log(n) - 1) = n × log(n)
   ✓ 验证通过
```

---

## 四、Tier 3: @unbounded 逃逸（Escape Hatch）

### 4.1 动机

某些递归函数的终止性或代价上界是未知的或不可判定的。
`@unbounded` 是显式声明："我知道此函数的代价无法静态确定"。

### 4.2 语法

```spore
@unbounded
fn collatz(n: Int) -> Int {
    match n {
        1 => 0,
        n if n % 2 == 0 => 1 + collatz(n / 2),
        n => 1 + collatz(3 * n + 1),
    }
}
```

### 4.3 规则

| 规则 | 描述 |
|------|------|
| 警告而非错误 | `@unbounded` 函数产生编译器 warning，不阻塞编译 |
| 传染性 | 调用 `@unbounded` 函数使调用者的代价也变为 `@unbounded`（除非包裹在代价限制器中） |
| 上下文限制 | `@unbounded` 函数不能在 `cost ≤ K` 上下文中直接使用 |
| Hole 交互 | `@unbounded` 函数内的 Hole 标记为 `cost_budget: unbounded` |

### 4.4 传染性与隔离

```spore
@unbounded
fn collatz(n: Int) -> Int { ... }

// ✗ 编译错误: 不能在 cost-bounded 上下文中直接调用 @unbounded 函数
fn analyze(n: Int) -> Int
    cost ≤ 1000
{
    collatz(n)  // ERROR [unbounded-in-bounded-context]
}

// ✓ 通过代价限制器包裹
fn safe_analyze(n: Int) -> Int ! [CostExceeded]
    cost ≤ 1000
{
    with_cost_limit(1000) {
        collatz(n)
    }
}

// ✓ 调用者也标记为 @unbounded
@unbounded
fn analyze_all(ns: List<Int>) -> List<Int> {
    ns |> map(collatz)
}
```

### 4.5 编译器警告格式

```
WARNING [unbounded-function] collatz 标记为 @unbounded。
  此函数的代价无法静态确定。
  调用链中的所有上游函数将继承 @unbounded 属性，
  除非通过 with_cost_limit 显式隔离。

  影响范围:
    → analyze_all (直接调用)
    → main (间接调用，经 analyze_all)
```

---

## 五、高阶函数的代价公式（Cost Formulas for Higher-Order Functions）

### 5.1 内置代价公式

由于 Spore 没有循环，高阶函数是迭代的**唯一**机制（除递归外）。
标准库中的高阶函数具有已知的代价公式，编译器内置识别：

| 函数 | 代价公式 | 各维度分解 |
|------|---------|-----------|
| `map(list, f)` | `len(list) × cost(f) + len(list)` | C: `n × C(f)`, A: `n` (新列表分配) |
| `fold(list, init, f)` | `len(list) × cost(f)` | C: `n × C(f)`, A: `0` (原地累积) |
| `filter(list, pred)` | `len(list) × cost(pred) + len(list)` | C: `n × C(pred)`, A: `≤ n` (最坏情况) |
| `flat_map(list, f)` | `len(list) × cost(f) + total_output_len` | C: `n × C(f)`, A: `output_len` |
| `zip(a, b)` | `min(len(a), len(b))` | C: `min(m, n)`, A: `min(m, n)` |
| `take(list, n)` | `n` | C: `n`, A: `n` |
| `reduce(list, f)` | `(len(list) - 1) × cost(f)` | C: `(n-1) × C(f)`, A: `0` |

### 5.2 纯函数保证

高阶函数的参数 `f` 在 Spore 中必须是纯函数（无 effect 变量）。
因此 `cost(f)` 在编译时**总是可静态确定的**：

```
∀ f: Fn(A) -> B where uses(f) = []:
    cost(f) 是编译时常量或符号表达式
```

这保证了高阶函数代价公式的组合性——
将 `cost(f)` 代入公式后，得到的仍是合法的符号代价表达式。

### 5.3 嵌套高阶函数

```spore
fn matrix_sum(matrix: List<List<Int>>) -> Int {
    matrix |> map(fn(row) { row |> fold(0, fn(a, b) { a + b }) })
           |> fold(0, fn(a, b) { a + b })
}
```

代价推导：
```
inner_fold = len(row) × cost(+) = len(row) × 1
map_step   = cost(inner_fold) = len(row) × 1
outer_map  = len(matrix) × map_step + len(matrix)
           = len(matrix) × len(row) + len(matrix)
outer_fold = len(matrix) × cost(+) = len(matrix) × 1
total      = outer_map + outer_fold
           = len(matrix) × len(row) + 2 × len(matrix)
           → O(m × n)
```

---

## 六、相互递归（Mutual Recursion）

### 6.1 检测

通过调用图的**强连通分量**（SCC, Strongly Connected Components）检测相互递归：

```
algorithm detect_mutual_recursion(call_graph):
    1. 计算调用图的 SCC（Tarjan 算法，O(V + E)）
    2. 每个大小 > 1 的 SCC 即为一组相互递归函数
    3. 对每个 SCC 作为整体进行递归分析
```

### 6.2 代价分析

将 SCC 中的所有函数视为**单一递归单元**，应用三层方案：

```spore
fn is_even(n: Int) -> Bool {
    match n {
        0 => true,
        n => is_odd(n - 1),
    }
}

fn is_odd(n: Int) -> Bool {
    match n {
        0 => false,
        n => is_even(n - 1),
    }
}
```

分析：
```
SCC = {is_even, is_odd}
组合调用模式: is_even(n) → is_odd(n-1) → is_even(n-2) → ...
参数 n 每两次调用递减 2 → 结构递归
cost(is_even, n) = cost(is_odd, n) = n × cost_per_step
→ O(n)
```

### 6.3 规则

| 场景 | 处理 |
|------|------|
| SCC 整体满足结构递归 | 自动推导代价 |
| SCC 不满足结构递归 | SCC 中**所有**函数需要 `cost ≤` 或 `@unbounded` |
| SCC 中部分函数有 `@unbounded` | 整个 SCC 视为 `@unbounded` |

---

## 七、与 Hole 系统的交互

### 7.1 递归函数中的 Hole

当 Hole 出现在递归函数内部时，HoleReport 需要考虑递归带来的代价开销：

```spore
fn process_tree<T>(tree: Tree<T>) -> Result<T>
    cost ≤ 500
{
    match tree {
        Leaf(v) => Ok(v),
        Node(left, val, right) => {
            l = process_tree(left)
            r = process_tree(right)
            ?combine_results    // Hole
        },
    }
}
```

### 7.2 HoleReport 输出

```json
{
  "hole": "combine_results",
  "expected_type": "Result<T>",
  "bindings": {
    "l": "Result<T>",
    "r": "Result<T>",
    "val": "T"
  },
  "recursion_context": {
    "pattern": "structural_binary_tree",
    "per_node_budget": "500 / nodes(tree)",
    "recursive_overhead": "nodes(tree) × (3[call] × 2)"
  },
  "cost_budget_remaining": "500 / nodes(tree) - 6",
  "note": "此 Hole 在每个树节点执行一次，填洞代价将乘以节点数"
}
```

### 7.3 各 Tier 下的 Hole 行为

| Tier | cost_budget_remaining | 说明 |
|------|----------------------|------|
| Tier 1 (结构递归) | `total_budget / iterations - recursive_overhead` | 按迭代次数均分预算 |
| Tier 2 (声明式) | `declared_bound / estimated_iterations - overhead` | 基于声明的上界计算 |
| Tier 3 (@unbounded) | `unbounded` | 无法计算预算 |

---

## 八、示例（Progressive Examples）

### 示例 1: 简单结构递归 — 阶乘

```spore
fn factorial(n: Int) -> Int {
    match n {
        0 => 1,
        n => n * factorial(n - 1),
    }
}
```

```
编译器分析:
  递归类型: 结构递归（n 每次递减 1）
  自动推导: cost = n × 4 (1[*] + 3[call])
  复杂度: O(n)
  ✓ 无需手动标注
```

### 示例 2: 树遍历 — 二叉结构递归

```spore
fn tree_sum(tree: Tree<Int>) -> Int {
    match tree {
        Leaf(v) => v,
        Node(left, val, right) => tree_sum(left) + val + tree_sum(right),
    }
}
```

```
编译器分析:
  递归类型: 结构递归（tree 递减为子树，双侧）
  自动推导: cost = nodes(tree) × 9 (1[+] + 1[+] + 3[call] × 2 + 1[match])
  复杂度: O(n)
  ✓ 无需手动标注
```

### 示例 3: 二分查找 — 对数结构递归

```spore
fn binary_search<T>(sorted: List<T>, target: T) -> Option<Int>
    where T: Ord
{
    search_helper(sorted, target, 0, len(sorted) - 1)
}

fn search_helper<T>(sorted: List<T>, target: T, low: Int, high: Int) -> Option<Int>
    where T: Ord
{
    match low > high {
        true => None,
        false => {
            mid = (low + high) / 2
            match compare(sorted[mid], target) {
                Equal => Some(mid),
                Less => search_helper(sorted, target, mid + 1, high),
                Greater => search_helper(sorted, target, low, mid - 1),
            }
        },
    }
}
```

```
编译器分析:
  递归类型: 结构递归（搜索区间 high - low 每次至少减半）
  自动推导: cost = log(high - low) × 12
  复杂度: O(log n)
  ✓ 无需手动标注
```

### 示例 4: 归并排序 — 声明式验证

```spore
fn merge_sort<T>(list: List<T>) -> List<T>
    where T: Ord
    decreases len(list)
    cost ≤ len(list) * log(len(list)) * 5 + len(list)
{
    match list {
        [] => [],
        [x] => [x],
        _ => {
            (left, right) = split_at(list, len(list) / 2)
            merge(merge_sort(left), merge_sort(right))
        },
    }
}
```

```
编译器验证:
  1. decreases len(list):
     ✓ len(left) = len(list) / 2 < len(list)
     ✓ len(right) = len(list) - len(list) / 2 < len(list)

  2. cost ≤ n × log(n) × 5 + n:
     body_cost = cost(split_at) + cost(merge) = O(n)
     recursive_cost ≤ 2 × ((n/2) × log(n/2) × 5 + n/2)
                    = n × log(n/2) × 5 + n
                    = n × (log(n) - 1) × 5 + n
                    = n × log(n) × 5 - 5n + n
                    = n × log(n) × 5 - 4n
     total ≤ n + n × log(n) × 5 - 4n = n × log(n) × 5 - 3n
           ≤ n × log(n) × 5 + n  ✓

  ✓ 代价上界验证通过
```

### 示例 5: Collatz 猜想 — @unbounded 逃逸

```spore
@unbounded
fn collatz_steps(n: Int) -> Int {
    match n {
        1 => 0,
        n if n % 2 == 0 => 1 + collatz_steps(n / 2),
        n => 1 + collatz_steps(3 * n + 1),
    }
}
```

```
编译器分析:
  递归类型: 非结构递归
  ✗ n / 2 递减，但 3 * n + 1 递增 → 无法证明终止
  @unbounded 已标注 → 产生 warning:

  WARNING [unbounded-function] collatz_steps 标记为 @unbounded。
    原因: 参数 n 在 3 * n + 1 分支上不满足递减条件
    影响: 调用此函数的所有上游函数将继承 @unbounded
```

---

## 九、判定边界（Decidability Boundary）

### 9.1 各层判定性

| 分析任务 | 可判定性 | 复杂度 | 说明 |
|----------|---------|--------|------|
| 结构递归检测 | ✓ 可判定 | O(\|call\_graph\|) | 语法层面检查参数递减 |
| 多项式代价验证 | ✓ 可判定 | O(poly\_degree) | 比较多项式系数 |
| 含 log 代价验证 | ✓ 可判定 | O(expr\_size) | 标准渐进比较 |
| 一般终止性 | ✗ 不可判定 | — | 停机问题的直接推论 |
| 一般代价上界 | ✗ 不可判定 | — | 蕴含终止性 |

### 9.2 Spore 的甜蜜点

```
                  理论可判定
         ┌────────────────────┐
         │                    │
         │   Tier 1: ~70%     │ ← 结构递归（完全自动）
         │                    │
         │   Tier 2: ~20%     │ ← 声明式（半自动验证）
         │                    │
         ├────────────────────┤
         │   Tier 3: ~10%     │ ← 不可判定区域（@unbounded）
         └────────────────────┘
                  理论不可判定
```

实践中，~90% 的代码落在可判定区域内。原因：

1. **无循环**：所有迭代通过高阶函数（已知代价）或显式递归（可分析）
2. **代数数据类型**：大量递归自然遵循数据结构（结构递归）
3. **纯函数**：无副作用意味着递归行为完全由参数决定

### 9.3 与其他语言的对比

| 语言 | 递归分析方式 | 覆盖率 |
|------|------------|--------|
| **Spore** | 三层组合（自动 + 声明 + 逃逸） | ~90% |
| Agda / Coq | 结构递归 + 良基递归（强制终止） | 100%（但拒绝不可判定程序） |
| Lean 4 | 结构递归 + `decreasing_by` + `partial` | ~95%（学术代码） |
| Rust / Go | 无静态递归分析 | 0%（依赖运行时栈溢出） |

Spore 选择了实用主义路线：不追求 100% 覆盖（那需要拒绝合法程序），
而是用 `@unbounded` 优雅地处理不可判定区域。

---

## 十、设计决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 递归分析层数 | 3 层 | 平衡自动化与表达力，~90% 覆盖率 |
| 结构递归判定方式 | 语法层面参数递减检查 | 简单、可靠、O(call_graph) 复杂度 |
| 验证失败处理 | warning（非 error） | 渐进式采用，不阻塞开发 |
| @unbounded 语义 | 传染性 + 可隔离 | 确保代价信息的传递性，同时提供逃逸路径 |
| decreases 子句 | 可选 | 大多数情况编译器可自动推导，仅在需要时手动提供 |
| 相互递归处理 | SCC 整体分析 | 与调用图分析自然契合 |
| 高阶函数代价 | 编译器内置公式 | 无循环 → 高阶函数是唯一迭代机制，必须内置 |

---

## 十一、开放问题

1. **尾调用优化与代价**：TCO 改变栈空间消耗但不改变计算代价。
   是否需要在代价模型中区分"栈深度"维度？
   当前方案：不区分，TCO 仅作为 codegen 优化，不影响代价分析。

2. **Memoization 与代价**：编译器是否应自动检测可 memo 化的递归并调整代价？
   例如 `fibonacci` 从 O(2^n) 优化为 O(n)。
   当前倾向：不自动 memo，但在警告信息中建议 memo 化。

3. **递归深度限制**：是否需要编译时强制递归深度上界？
   当前方案：仅在 `@unbounded` 函数的运行时通过 `with_cost_limit` 限制。

4. **概率终止**：某些随机化算法（如 QuickSort 随机选 pivot）的代价是期望值。
   是否支持 `cost ≤ O(n log n) expected`？
   留待后续版本设计。
