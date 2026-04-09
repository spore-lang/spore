# Spore 代价分析综合规范 (Cost Analysis Spec) v0.1

> **Status**: Draft
> **Scope**: 编译器代价分析、类型检查、Hole 系统
> **日期**: 2025-07
>
> 本文档整合了以下三份规范：
> - Part I: 代价模型 (原 cost-model-v0.1.md)
> - Part II: 代价表达式文法与可判定性 (原 cost-decidability-v0.1.md)
> - Part III: 递归代价分析 (原 recursion-analysis-v0.1.md)

---

# Part I: 模拟执行与抽象代价模型

## 一、核心理念

传统语言的性能分析依赖：运行时 profiling → 机器相关 → 不可复现。
我们的方案：**编译时模拟执行** → 机器无关 → 确定性可复现。

核心等式：
```
模拟执行 = 抽象解释(Abstract Interpretation) + 确定性代价表
```

> **Active surface syntax**：源码中的代价声明统一写成 `cost [compute, alloc, io, parallel]`。旧的 `cost <= expr` 标量写法已经移除；下文若出现 `log`/`max`/`min` 等 richer 标量表达式，均指内部分析记法或未来讨论，不是当前源码规范。
>
> **记法约定**：在本语言中 `log` 统一指 `log₂`（二进制对数），符合计算机科学惯例。

编译器不运行真实代码，而是在抽象机器上"走一遍"程序，
每一步累加确定性代价。结果是一个精确的（或有界的）代价值。

---

## 二、抽象机器模型（Abstract Machine）

### 2.1 代价维度

内部维护四个独立维度：

| 维度 | 缩写 | 含义 | 单位名 |
|------|------|------|--------|
| 计算 | `C` | CPU 操作步数 | op（operation） |
| 分配 | `A` | 堆内存分配量 | cell（抽象内存单元） |
| 读写 | `W` | IO/副作用操作次数 | call（外部调用次数） |
| 并发宽度 | `P` | 并行执行通道数 | lane（并行通道） |

**签名层面**：源码固定声明 `cost [compute, alloc, io, parallel]`，四个槽位按 C/A/W/P 顺序出现。
**查询层面**：`sporec --query-cost fn_name` 返回完整四维明细 + 分解；工具可额外提供派生的标量汇总，但那不是规范化源码语法。

### 2.2 标量折算公式

```
cost = C × 1 + A × α + W × β
```

其中 `α`, `β` 是项目级可配置权重（在 `spore.toml` 里设定）：

```toml
[cost]
alloc_weight = 2      # α: 每个 cell 折算为 2 op
io_weight = 100       # β: 每次外部调用折算为 100 op
```

默认值 `α=2, β=100` 反映"分配比计算略贵，IO 远贵于计算"的通用假设。
项目可以根据场景调整（如高频分配场景增大 α）。

---

## 三、原语操作代价表

### 3.1 计算操作（C 维度）

| 操作类别 | 操作 | 代价 (op) | 说明 |
|----------|------|----------|------|
| 整数算术 | `+`, `-`, `*` | 1 | |
| 整数除法 | `/`, `%` | 2 | 除法比乘法略贵 |
| 浮点算术 | `+.`, `-.`, `*.` | 2 | |
| 浮点除法 | `/.` | 3 | |
| 比较操作 | `==`, `!=`, `<`, `>` | 1 | |
| 逻辑操作 | `&&`, `||`, `!` | 1 | 短路不影响代价（取最大路径） |
| 位操作 | `&`, `|`, `^`, `<<`, `>>` | 1 | |
| 变量读取 | 栈变量 | 0 | 已在作用域 |
| 变量绑定 | `let x = ...` | 1 | |
| 模式匹配 | 每个 pattern arm | 1 | 取实际匹配的分支 |
| 函数调用 | 调用开销 | 3 | 固定开销，不含被调函数体代价 |
| 闭包创建 | 捕获 N 个变量 | N + 2 | |
| 管道操作 | `|>` | 0 | 语法糖，不增加代价 |

### 3.2 分配操作（A 维度）

| 操作 | 代价 (cell) | 说明 |
|------|------------|------|
| 创建结构体 | 字段数 | 每字段 1 cell |
| 创建列表 | 元素数 | 每元素 1 cell + 1 header |
| 字符串创建 | ⌈len/8⌉ | 每 8 字符 1 cell |
| 字符串拼接 | ⌈(len_a + len_b)/8⌉ | 新分配 |
| 枚举/联合类型 | 1 | tag + 最大变体大小 |
| 拷贝 | 原值 cell 数 | 深拷贝 |
| 引用（借用） | 0 | 无分配 |

### 3.3 IO/副作用操作（W 维度）

| 操作 | 代价 (call) | 说明 |
|------|------------|------|
| 文件读/写 | 1 | 每次系统调用 |
| 网络请求 | 1 | 每次请求 |
| 随机数生成 | 1 | 每次调用 |
| 时钟读取 | 1 | 每次调用 |
| 标准输入/输出 | 1 | 每次调用 |
| 状态读/写 | 1 | 每次可变状态访问 |

---

## 四、复合表达式的代价组合规则

### 4.1 顺序执行
```
cost(A; B) = cost(A) + cost(B)
```

### 4.2 条件分支
```
cost(if c then A else B) = cost(c) + max(cost(A), cost(B))
```
取**最大路径**——代价上界是保守的（sound）。

### 4.3 模式匹配
```
cost(match x { p1 => A, p2 => B, ... }) = cost(x) + max(cost(A), cost(B), ...)
                                          + (匹配 arm 数 × 1)
```

### 4.4 函数调用
```
cost(f(args)) = sum(cost(arg_i)) + 3(调用开销) + cost(f.body)
```
被调函数的代价从其编译结果获取（已知）或从其签名声明获取。

### 4.5 管道链
```
cost(x |> f |> g |> h) = cost(x) + cost(f) + cost(g) + cost(h)
```

---

## 五、循环与递归的代价分析

这是最关键也最困难的部分。

### 5.1 有界迭代（fold/map/filter）

```
fn sum_list(items: List[I32]) -> I32 {
    items |> fold(start: 0, step: fn(acc, x) -> acc + x)
}
```

编译器知道 `fold` 遍历整个列表，所以：
```
cost = N × cost(step) + cost(fold_overhead)
```
其中 `N` 是集合大小。

**关键：** 集合大小在编译时通常未知。此时代价表达为**符号表达式**：

```
cost(sum_list) = N × 2 + 5    -- 其中 N = len(items)
```

签名中的 `cost [compute, alloc, io, parallel]` 约束此时意味着：
- 编译器分别验证四个槽位在给定 N 范围内成立
- 如果参数类型带有大小约束（如 `List[I32, max: 1000]`），编译器可以验证
- 如果无大小约束，编译器要求开发者明确声明

### 5.2 有界类型（Bounded Types）

为了让代价系统在编译时可验证，引入大小约束类型：

```
fn process_batch(items: List[Order, max: 500]) -> BatchResult ! [TooLarge]
    cost [25000, 0, 0, 0]
    uses [Compute]
{
    ...
}
```

`List[Order, max: 500]` 表示编译器保证此列表最多 500 个元素。
超过时在调用点产生 `TooLarge` 错误（由 runtime 检查，但代价分析在编译时以 500 为上界）。

### 5.3 结构递归（Structural Recursion）

结构递归（每次递归参数严格变小）是可证终止的，代价可以推导：

```
fn tree_depth[T](tree: Tree[T]) -> I32 {
    match tree {
        Leaf(_) => 0
        Node(left, _, right) => 1 + max(tree_depth(left), tree_depth(right))
    }
}
```

代价 = `D × (cost_per_level)` 其中 D = 树深度。
编译器要求 `Tree[T]` 有 `max_depth` 约束或代价为符号表达式。

### 5.4 一般递归

无法证明终止 → 无法静态确定代价。处理方式：

```
fn fibonacci(n: I32) -> I32 {
    match n {
        0 => 0
        1 => 1
        _ => fibonacci(n - 1) + fibonacci(n - 2)
    }
}
```

编译器输出：
```
WARNING [unbounded-cost] fibonacci 的代价无法静态确定。
  递归模式: 非结构递归
  推断复杂度: O(2^n)

  选项:
  (a) 添加 `cost [compute, alloc, io, parallel]` + 参数约束 `n: I32 if n ≤ 30`
  (b) 标记为 `unbounded`（放弃代价约束）
  (c) 改用结构递归或尾递归 + 迭代上限
```

编译器会把这类函数标记为 `unbounded`。显式的源码声明拼写尚未纳入当前规范；当前文档只保留该分析状态，而不定义新的表面语法。

`unbounded` 函数**不能被已声明 `cost [compute, alloc, io, parallel]` 的函数调用**，除非包裹在运行时代价限制器中：
```
fn safe_fib(n: I32) -> I32 ! [CostExceeded]
    cost [10000, 0, 0, 0]
    uses [Compute]
{
    with_cost_limit(10000) {
        fibonacci(n)
    }
}
```

---

## 六、符号代价表达式

当代价依赖输入大小时，编译器维护符号表达式而非具体数字：

```
fn sort[T](items: List[T, max: N]) -> List[T, max: N]
    where T: Ord
    cost [O(N log N), O(N), 0, 0]
    uses [Compute]
{
    ...
}
```

当前源码示例只把 `O(·)` 级别的摘要写进向量槽位。更丰富的符号表达式仍用于编译器内部分析；若未来把它们开放到源码层，将另行立规范。

内部符号代价支持的操作符：
- 算术：`+`, `×`, `-`
- 对数：`log(N)`
- 幂：`N^k`（k 为常数）
- min/max：`max(expr1, expr2)`
- 参数引用：`len(param_name)` 或类型约束中的 `max` 值

编译器验证时：
1. 从类型约束提取变量上界（如 `N ≤ 1000`）
2. 将符号表达式实例化
3. 验证 `实例化代价 ≤ 声明上界`

---

## 七、模拟执行流程

### 7.1 编译时流程

```
源代码
  ↓
[1] 解析为 AST
  ↓
[2] 类型检查 + 效果检查 + 能力检查
  ↓
[3] 抽象解释（Abstract Interpretation）
    ├── 构建控制流图（CFG）
    ├── 对每个基本块计算代价
    ├── 对分支取 max
    ├── 对迭代/递归分析上界
    └── 生成符号代价表达式
  ↓
[4] 代价验证
    ├── 与签名中的四维 `cost [...]` 各槽位比对
    ├── 通过 → 编译成功
    ├── 超出 → 编译错误 + 详细分解
    └── 无法确定 → 警告 + 建议
  ↓
[5] 代价信息写入编译输出（JSON / 二进制）
```

### 7.2 编译器查询接口

```bash
# 查看函数代价概要
$ sporec --query-cost parse_config
{
  "function": "parse_config",
  "cost_scalar": 187,
  "cost_declared": "[200, 3, 0, 0]",
  "status": "within_bound",
  "dimensions": { "compute": 180, "alloc": 3, "io": 0, "parallel": 0 },
  "breakdown": [
    { "call": "toml.parse",     "compute": 120, "alloc": 2, "io": 0, "parallel": 0 },
    { "call": "validate_keys",  "compute": 45,  "alloc": 1, "io": 0, "parallel": 0 },
    { "expr": "self",           "compute": 15,  "alloc": 0, "io": 0, "parallel": 0 }
  ]
}

# 查看符号代价（泛型/参数化函数）
$ sporec --query-cost sort
{
  "function": "sort",
  "cost_symbolic": { "compute": "O(N log N)", "alloc": "O(N)", "io": 0, "parallel": 0 },
  "cost_declared": "[O(N log N), O(N), 0, 0]",
  "variables": { "N": "len(items), max = param constraint" },
  "status": "symbolic_match"
}
```

---

## 八、与 Hole 系统的交互

部分定义的函数也参与代价分析：

```
fn process(data: Data) -> Result ! [ProcessError]
    cost [1000, 0, 0, 0]
    uses [Compute]
{
    parsed = parser.parse(data)     -- cost: 600
    ?process_logic                  -- hole: 剩余预算 400
}
```

模拟执行到 Hole 时：
```json
{
  "hole": "process_logic",
  "cost_consumed": 600,
  "cost_budget_remaining": 400,
  "note": "填洞实现的代价不能超过 400 op"
}
```

这给 Agent 填洞时提供了精确的约束：不仅知道类型要求，还知道性能预算。

---

## 九、与能力系统的交互

能力声明影响代价计算：

- `uses []`（自由函数） → W 维度必须为 0
- `uses [Compute]` → W 维度必须为 0（Compute 不含 IO）
- `uses [NetRead]` → W 维度可 > 0

如果编译器推断函数为 `pure`（基于 `uses`）但代价分析发现 W > 0 → 编译错误。
这形成了一个交叉验证网络：**能力 ↔ 代价 互相约束**。

---

## 十、开放问题

1. **多态函数的代价**：泛型函数（如 `map(f, list)`）不尝试预测多态代价。
   - 泛型函数的代价在**调用点**通过模拟执行计算：编译器将具体类型参数代入后重新执行代价分析。
   - 签名层面只需为特定类型约束声明代价上界，例如：
     ```
     fn map[T, U](f: T -> U, items: List[T, max: N]) -> List[U, max: N]
         where T: Sized, U: Sized
         cost [N × cost(f) + N, 0, 0, 0]
     ```
   - `cost(f)` 在每个调用点已知（因为 `f` 已被具体化），编译器直接代入计算即可。
   - 不支持对未具体化的高阶类型参数进行抽象代价推理——这保持了系统的简单性和确定性。

2. **并发/异步的代价模型**：第 4 维度**并发宽度**（`P`）的组合规则。

   - 顺序代价维度（C, A, W）在并行分支间取 `max`：
     ```
     cost(parallel { A, B }) = max(cost(A), cost(B))  -- 对 C, A, W 各维度分别取 max
     ```
   - 并发宽度 `P` 取 `sum`（总并行通道数 = 各分支并行度之和）。
   - 总墙钟等价代价 = `max(各分支代价) + 同步开销`（同步开销为常数，取决于并发原语）。
   - 标量折算时，`P` 不参与加权求和——它作为独立维度报告，用于资源规划而非性能预测。

3. **外部函数（FFI）**：外部函数**必须**在绑定时声明代价。语法如下：
   ```
   extern fn openssl_encrypt(data: Bytes, key: Key) -> Bytes ! [CryptoError]
       cost [500, 0, 0, 0]
       uses [Compute]
   ```
   - 如果外部函数未声明代价，则视为 `unbounded`，与未标注代价的一般递归函数受相同限制：不能被已声明 `cost [compute, alloc, io, parallel]` 的函数直接调用。
   - 这确保了 FFI 边界的代价透明性——外部世界不能静默地引入代价黑洞。

4. **代价漂移检测**（Future Work）：实际运行时代价与编译时预测的偏差监控。
   - 此问题可通过测试/验证系统部分解决：运行时对代价进行采样，与编译时预测值进行对比。
   - 可配置容差阈值（如 `cost_drift_tolerance = 1.2` 表示允许 20% 偏差），超出阈值时报告警告。
   - 留待后续版本设计完整的运行时代价采样框架。

---

# Part II: 代价表达式文法与可判定性证明

## 一、概述

Spore 的代价系统允许函数声明 `cost [compute, alloc, io, parallel]`。Part II 研究的是各个槽位背后的内部符号表达式与可判定性；这些表达式并不是当前源码层直接书写的标量语法。编译器需要完成以下验证任务：

> **验证问题**：给定函数体的推断代价 C(n̄) 和开发者声明的上界 B(n̄)，判定是否对所有合法输入 n̄ 都有 C(n̄) ≤ B(n̄)。

这要求：

1. **可判定性（Decidability）**：验证问题必须是可判定的——编译器必须在有限时间内给出"通过"或"不通过"的确定答案，而非陷入无限循环。
2. **高效性（Efficiency）**：验证应在多项式时间内完成，不能成为编译的瓶颈。

本规范的目标是：

- 严格定义 **CostExpr 文法**，使得渐近比较在多项式时间内可判定
- 给出 **标准形转换算法** 和 **支配性判定算法**
- 证明整体验证算法的 **时间复杂度为 P**（多项式时间）
- 扩展至四维代价模型和递归分析的交互

---

## 二、CostExpr 文法

### 2.1 BNF 文法定义

```bnf
CostExpr ::= Literal                              (* 正整数常量: 1, 100, 1000 *)
           | Var                                    (* 变量: n, m, k — 来自函数参数的大小信息 *)
           | CostExpr '+' CostExpr                 (* 加法 *)
           | CostExpr '*' CostExpr                 (* 乘法 *)
           | CostExpr '^' Literal                  (* 幂 — 指数必须是正整数常量 *)
           | 'log' '(' CostExpr ')'                (* 对数 — base 2, 上取整 *)
           | 'max' '(' CostExpr ',' CostExpr ')'   (* 取较大值 *)
           | 'min' '(' CostExpr ',' CostExpr ')'   (* 取较小值 *)

Literal  ::= [1-9][0-9]*                           (* 正整数，不含 0 *)
Var      ::= [a-z][a-z0-9_]*                       (* 小写字母开头的标识符 *)
```

### 2.2 明确禁止的构造

| 禁止的构造 | 禁止原因 |
|------------|---------|
| 除法 `/` | 避免除零问题和有理数表达式，保证表达式在 ℕ⁺ 上封闭 |
| 减法 `-` | 代价始终非负；减法可能产生负值，破坏单调性 |
| 条件表达式 `if ... then ... else ...` | 引入不可判定的分支——条件可以编码任意谓词 |
| 递归定义的代价函数 | 避免不动点计算；递归代价的分析在递归分析层完成（见第八节） |
| 负数 | 代价空间为 ℕ（非负整数），负数无物理意义 |
| 变量指数 `n^m` | 将比较问题推入指数多项式域，丧失多项式时间可判定性 |

### 2.3 设计原理

文法的设计遵循一个核心原则：**表达力与可判定性的平衡**。

- **多项式-对数（poly-log）表达式** 覆盖了绝大多数实际算法的复杂度类：O(1)、O(log n)、O(n)、O(n log n)、O(n²)、O(n² log n) 等。
- 禁止除法和减法保证了表达式在 ℕ⁺ 上的 **封闭性** 和 **单调性**。
- 禁止条件和递归保证了 **可判定性**——没有图灵完备的子语言。

---

## 三、语义定义

### 3.1 求值域

CostExpr 在 **ℕ⁺ = {1, 2, 3, ...}**（正自然数）上求值。

设 σ: Var → ℕ⁺ 为一个赋值环境，则语义函数 ⟦·⟧: CostExpr → (Var → ℕ⁺) → ℕ⁺ 定义如下：

```
⟦ c ⟧σ           = c                              (c ∈ ℕ⁺)
⟦ x ⟧σ           = σ(x)                           (x ∈ Var)
⟦ e₁ + e₂ ⟧σ    = ⟦e₁⟧σ + ⟦e₂⟧σ
⟦ e₁ * e₂ ⟧σ    = ⟦e₁⟧σ × ⟦e₂⟧σ
⟦ e ^ k ⟧σ      = (⟦e⟧σ)ᵏ                        (k ∈ ℕ⁺)
⟦ log(e) ⟧σ     = ⌈log₂(⟦e⟧σ)⌉                  (特别地, log(1) = 0 → 取 max(⌈log₂(·)⌉, 1))
⟦ max(e₁,e₂) ⟧σ = max(⟦e₁⟧σ, ⟦e₂⟧σ)
⟦ min(e₁,e₂) ⟧σ = min(⟦e₁⟧σ, ⟦e₂⟧σ)
```

> **注**：`log(1)` 的数学值为 0，但在代价上下文中我们定义 `log(e)` 的最小值为 1，即 `⟦log(e)⟧σ = max(⌈log₂(⟦e⟧σ)⌉, 1)`。这避免了乘以 0 导致信息丢失的问题。

### 3.2 单调性定理

**定理 3.1（单调性）**：对任意 CostExpr `e`，若对所有变量 x 有 σ₁(x) ≤ σ₂(x)，则 ⟦e⟧σ₁ ≤ ⟦e⟧σ₂。

**证明**（结构归纳）：

- **基础情况**：
  - `Literal c`：⟦c⟧σ = c，与 σ 无关，显然成立。
  - `Var x`：⟦x⟧σ₁ = σ₁(x) ≤ σ₂(x) = ⟦x⟧σ₂，由假设直接得出。

- **归纳步骤**（假设子表达式满足单调性）：
  - `e₁ + e₂`：⟦e₁⟧σ₁ + ⟦e₂⟧σ₁ ≤ ⟦e₁⟧σ₂ + ⟦e₂⟧σ₂，由归纳假设和加法单调性。
  - `e₁ * e₂`：在 ℕ⁺ 上，乘法对两个非负因子单调。由归纳假设 ⟦eᵢ⟧σ₁ ≤ ⟦eᵢ⟧σ₂，且所有值 ≥ 1，故 ⟦e₁⟧σ₁ × ⟦e₂⟧σ₁ ≤ ⟦e₁⟧σ₂ × ⟦e₂⟧σ₂。
  - `e ^ k`：x ↦ xᵏ 在 ℕ⁺ 上单调递增，由归纳假设得证。
  - `log(e)`：⌈log₂(·)⌉ 在 ℕ⁺ 上单调非递减，由归纳假设得证。
  - `max(e₁, e₂)`：若 ⟦eᵢ⟧σ₁ ≤ ⟦eᵢ⟧σ₂ (i=1,2)，则 max(⟦e₁⟧σ₁, ⟦e₂⟧σ₁) ≤ max(⟦e₁⟧σ₂, ⟦e₂⟧σ₂)。
  - `min(e₁, e₂)`：类似，min 在两个参数同时增大时非递减。 ∎

### 3.3 单调性的意义

单调性保证了：**如果不等式对所有"足够大"的输入成立，则对所有输入成立**（在适当调整常数后）。这是渐近比较算法的理论基础。

---

## 四、渐近比较算法

### 4.0 问题定义

**输入**：两个 CostExpr——推断代价 C 和声明上界 B，以及变量集合 V = {n₁, ..., nₖ}。

**判定问题**：是否存在常数 N₀ ∈ ℕ⁺，使得对所有 n̄ ∈ (ℕ⁺)ᵏ 且 nᵢ ≥ N₀ (∀i)，都有 ⟦C⟧σ_n̄ ≤ ⟦B⟧σ_n̄？

**输出**：`PASS`（确认 C 渐近不超过 B）或 `FAIL`（存在反例或无法确认）。

### 4.1 标准形（Normal Form）

#### 4.1.1 单项式定义

一个 **多项式-对数单项式（poly-log monomial）** 具有以下形式：

```
t = c × n₁^a₁ × n₂^a₂ × ... × nₖ^aₖ × log(n₁)^b₁ × log(n₂)^b₂ × ... × log(nₖ)^bₖ
```

其中：
- c ∈ ℕ⁺ 为正整数系数
- aᵢ ∈ ℕ（非负整数）为多项式指数
- bᵢ ∈ ℕ（非负整数）为对数指数

我们将单项式记作三元组 **(c, ā, b̄)**，其中 ā = (a₁,...,aₖ), b̄ = (b₁,...,bₖ)。

#### 4.1.2 标准形定义

CostExpr 的 **标准形** 是一个单项式的有限和：

```
NF(e) = t₁ + t₂ + ... + tₘ
```

其中每个 tⱼ = (cⱼ, āⱼ, b̄ⱼ) 是一个多项式-对数单项式。

#### 4.1.3 标准形转换算法

**算法 NF**：CostExpr → 标准形

```
NF(c)           = {(c, 0̄, 0̄)}                              -- 常量
NF(nᵢ)         = {(1, eᵢ, 0̄)}                              -- 变量 (eᵢ 是第 i 个位置为 1 的单位向量)
NF(e₁ + e₂)    = NF(e₁) ∪ NF(e₂)                           -- 加法：合并单项式集
NF(e₁ * e₂)    = {(c₁c₂, ā₁+ā₂, b̄₁+b̄₂) |                 -- 乘法：分配律展开
                   (c₁,ā₁,b̄₁) ∈ NF(e₁),
                   (c₂,ā₂,b̄₂) ∈ NF(e₂)}
NF(e ^ k)      = NF(e *ᵏ e)                                  -- 幂：展开为 k 次乘法
NF(log(nᵢ))    = {(1, 0̄, eᵢ)}                              -- 对单个变量的 log
NF(log(e))      = 详见下文「对数处理」                         -- 对复合表达式的 log
NF(max(e₁,e₂)) = 详见 4.3 节                                 -- max 需要特殊处理
NF(min(e₁,e₂)) = 详见 4.3 节                                 -- min 需要特殊处理
```

**对数处理**：当 `log` 应用于复合表达式时，利用以下渐近等价进行简化：

```
log(e₁ * e₂) ≈ log(e₁) + log(e₂)           -- 对数的乘法性质
log(e ^ k)   ≈ k × log(e)                   -- 对数的幂性质
log(e₁ + e₂) ≈ log(max(e₁, e₂))            -- 渐近等价：和的对数 ≈ 最大项的对数
log(c)       = ⌈log₂(c)⌉                     -- 常量直接计算
```

对于 `log(nᵢ^aᵢ)`，化简为 `aᵢ × log(nᵢ)`，即 `(aᵢ, 0̄, eᵢ)`。

> **注意**：这些是渐近等价，非精确等价。在渐近比较中这是合法的——我们关心的是"足够大"的输入。

#### 4.1.4 合并同类项

转换完成后，对标准形进行合并：具有相同 (ā, b̄) 的单项式合并系数：

```
(c₁, ā, b̄) + (c₂, ā, b̄) → (c₁ + c₂, ā, b̄)
```

### 4.2 渐近支配关系与比较算法

#### 4.2.1 单项式渐近序

**定义 4.1（渐近支配）**：单项式 t₁ = (c₁, ā₁, b̄₁) 被 t₂ = (c₂, ā₂, b̄₂) **渐近支配**（记作 t₁ ≼ t₂），当且仅当以下条件 **全部** 满足：

1. **多项式指数**：对所有 i，a₁ᵢ ≤ a₂ᵢ
2. **对数指数**（当多项式指数相等时）：若 ā₁ = ā₂，则对所有 i，b₁ᵢ ≤ b₂ᵢ
3. **系数**（当指数完全相等时）：若 ā₁ = ā₂ 且 b̄₁ = b̄₂，则 c₁ ≤ c₂

更精确地说，渐近支配关系按层级判定：

```
t₁ ≼ t₂  ⟺  ā₁ < ā₂ (存在某维严格小且其余不大于)
           ∨  (ā₁ = ā₂ ∧ b̄₁ < b̄₂)
           ∨  (ā₁ = ā₂ ∧ b̄₁ = b̄₂ ∧ c₁ ≤ c₂)
```

其中 ā₁ < ā₂ 表示 ā₁ ≤ ā₂ 逐分量（componentwise）且 ā₁ ≠ ā₂。

#### 4.2.2 标准形比较算法

**算法 COMPARE**：给定标准形 NF(C) = {s₁, ..., sₚ} 和 NF(B) = {t₁, ..., tᵧ}，判定 C ≤ B 是否渐近成立。

```
COMPARE(NF(C), NF(B)):
  1. 对 NF(C) 和 NF(B) 分别合并同类项
  2. 对 NF(C) 中的每个单项式 sⱼ:
       找到 NF(B) 中渐近增长率最高的匹配项 tₖ 使得 sⱼ 可被 tₖ 吸收
       「吸收」定义：
         a. 若 sⱼ 的 (ā, b̄) 严格小于某个 tₖ 的 (ā, b̄)，则 sⱼ 被渐近支配（无论系数）
         b. 若 sⱼ 的 (ā, b̄) 等于某个 tₖ 的 (ā, b̄)，则检查系数 cⱼ ≤ cₖ
  3. 若所有 sⱼ 都被吸收 → 返回 PASS
  4. 否则 → 返回 FAIL
```

**直觉**：这个算法逐项检查——C 的每一个增长项是否都被 B 的某个增长项"兜住"。

#### 4.2.3 算法正确性简述

**引理 4.1**：若 NF(C) 的每个单项式都被 NF(B) 的某个单项式渐近支配（支配关系允许 NF(B) 的一个单项式支配 NF(C) 的多个单项式），则存在 N₀ 使得对所有 nᵢ ≥ N₀，有 C(n̄) ≤ B(n̄)。

> 证明草案：当多项式指数严格支配时，高阶项的增长速度保证了在足够大的输入下完全覆盖低阶项。当指数相等时退化为系数比较，这是精确的。∎

### 4.3 max/min 的处理

`max` 和 `min` 不能直接展开为标准形的多项式-对数和，需要特殊规则。

#### 4.3.1 作为被验证表达式（C 中出现 max/min）

| 形式 | 验证规则 | 正确性 |
|------|---------|--------|
| `max(A, B) ≤ C` | 验证 A ≤ C **且** B ≤ C | 充要条件 |
| `min(A, B) ≤ C` | 验证 A ≤ C **或** B ≤ C | 充分条件（保守） |

**解释**：`max(A,B) ≤ C` 要求两个分支都不超过上界。`min(A,B) ≤ C` 只要较小的那个不超过上界即已满足，但编译器保守地只要有一个通过即接受。

#### 4.3.2 作为声明上界（B 中出现 max/min）

| 形式 | 验证规则 | 正确性 |
|------|---------|--------|
| `C ≤ max(A, B)` | 验证 C ≤ A **或** C ≤ B | 充分条件（保守） |
| `C ≤ min(A, B)` | 验证 C ≤ A **且** C ≤ B | 充要条件 |

**解释**：`C ≤ max(A,B)` 只要 C 不超过两者之一即可。`C ≤ min(A,B)` 要求 C 同时不超过 A 和 B。

#### 4.3.3 递归处理

当 max/min 嵌套在更深的表达式中时，先将它们"提升"到顶层：

```
e₁ + max(e₂, e₃) → max(e₁ + e₂, e₁ + e₃)    -- max 对加法分配
e₁ * max(e₂, e₃) → max(e₁ * e₂, e₁ * e₃)    -- max 对乘法分配（因 e₁ ≥ 1）
```

这种提升可能导致表达式膨胀，但由于 max/min 的嵌套深度在实际代码中有界（通常 ≤ 3），膨胀是可控的。

---

## 五、复杂度证明

**定理 5.1（多项式时间可判定性）**：CostExpr 的渐近比较问题可在多项式时间内判定。

**证明**：

设输入为两个 CostExpr C 和 B，令 n = |C| + |B| 为表达式的总大小（AST 节点数），k = |V| 为变量数。

#### 第一步：max/min 提升

将 max/min 提升到顶层，产生一组无 max/min 的子表达式对。

- max/min 的嵌套深度设为 d（文法限制 d 为常数，实践中 d ≤ 3）
- 提升后子表达式对数量 ≤ 2ᵈ = O(1)
- 每对子表达式大小 ≤ n × 2ᵈ = O(n)

> **若不对 d 做常数限制**：最坏情况下子表达式对数量为 2^{O(n)}，超出多项式。因此我们在文法层面限制 max/min 的嵌套深度（编译器可配置上限，默认为 8 层）。在 d 为常数时，此步骤为 O(n)。

#### 第二步：标准形转换

对每个无 max/min 的表达式转换为标准形。

- 乘法展开（分配律）：一个大小为 n 的表达式展开后单项式数量上界为 O(n²)（因为乘法最多将两个和式的项做笛卡尔积；幂 `e^k` 展开为 k 次乘法，但 k 是常量级别的 Literal）
- 对数化简：O(n) 次替换
- 合并同类项：排序 O(n² log n²) = O(n² log n)

**单步复杂度**：O(n² log n)

#### 第三步：支配性检查

对标准形 NF(C) 的每个单项式，在 NF(B) 中寻找支配项。

- NF(C) 的单项式数：p ≤ O(n²)
- NF(B) 的单项式数：q ≤ O(n²)
- 比较一对单项式：O(k)（逐变量比较指数向量）

**单步复杂度**：O(p × q × k) = O(n⁴ × k)

#### 总复杂度

```
T(n, k) = O(1) × [O(n² log n) + O(n⁴ × k)]
         = O(n⁴ × k)
```

当 k 视为输入的一部分时（k ≤ n），总复杂度为 **O(n⁵)**。

**结论**：验证算法的时间复杂度为 O(n⁵)，属于 **P**（多项式时间复杂度类）。 ∎

---

## 六、正确性证明草案

### 6.1 可靠性（Soundness）

**定理 6.1（可靠性）**：若算法输出 PASS，则存在常数 N₀ ∈ ℕ⁺，使得对所有 n̄ ∈ (ℕ⁺)ᵏ 且 min(n̄) ≥ N₀，有 C(n̄) ≤ B(n̄)。

**证明草案**：

1. 标准形转换保持渐近等价性（对数化简的误差在常数因子以内）。
2. 若 NF(C) 的每个单项式被 NF(B) 的某个单项式渐近支配：
   - 当支配是严格的（多项式指数严格小于），存在 N₁ 使得超过 N₁ 后高阶项完全覆盖。
   - 当支配是精确的（指数相等，系数 ≤），则对所有输入都成立。
3. 取 N₀ = max(所有单项式对的 Nᵢ)，则对 min(n̄) ≥ N₀ 全局成立。 ∎

### 6.2 小输入的处理

对于 n̄ < N₀ 的"小输入"，编译器采用 **枚举验证**：

```
for each n̄ ∈ {1, ..., N₀-1}ᵏ:
    assert C(n̄) ≤ B(n̄)
```

N₀ 可从支配性分析中的系数计算得出，通常 N₀ 较小（≤ 100）。枚举空间为 N₀ᵏ，在变量数 k 有限（通常 k ≤ 5）时是可行的。

### 6.3 保守性（Conservatism）

算法可能在以下情况返回 FAIL，但实际上不等式成立：

- 项之间的"消去"效应（如 `n² + n` ≤ `2n²` 成立但单项式 `n` 在 NF(B) 中无直接对应项）

**缓解措施**：在简单的支配性检查失败后，编译器可退化到更精细的比较：

1. 将 NF(C) 中未被吸收的项 **求和**，检查总和是否被 NF(B) 的最大项支配
2. 如果仍然失败，尝试数值抽样验证（在若干个大值点上计算）

这些额外步骤增加了通过率，但不影响可靠性（额外步骤只可能将 FAIL 转为 PASS，不会反向）。

---

## 七、四维代价的扩展

### 7.1 四维代价模型回顾

根据 Part I (代价模型) 第十节的定义，完整的代价向量为：

| 维度 | 缩写 | 含义 | 单位 |
|------|------|------|------|
| 计算 | `C` | CPU 操作步数 | op |
| 分配 | `A` | 堆内存分配量 | cell |
| 读写 | `W` | IO/副作用操作次数 | call |
| 并发宽度 | `P` | 并行执行通道数 | lane |

### 7.2 独立验证

四维代价的验证是 **逐维独立** 的。声明：

```
cost [O(n log n), O(n), 0, 4]
```

等价于四个独立的验证问题：

```
C_compute(n̄)  ≤  n * log(n)     -- 验证问题 1
C_alloc(n̄)    ≤  n               -- 验证问题 2
C_io(n̄)       ≤  0               -- 验证问题 3（特殊：零上界）
C_parallel(n̄) ≤  4               -- 验证问题 4（特殊：常数上界）
```

每个维度使用本规范定义的同一套算法独立判定。

### 7.3 派生标量视图的验证

当工具生成派生的标量汇总视图时：

```
cost = C×1 + A×α + W×β + P×γ
```

编译器首先计算各维度的符号代价表达式，然后合成标量表达式：

```
scalar_cost(n̄) = C_compute(n̄) + α × C_alloc(n̄) + β × C_io(n̄) + γ × C_parallel(n̄)
```

其中 α, β, γ 为项目配置的常数权重。合成后的标量表达式仍然是合法的 CostExpr（常数乘法和加法），因此可直接使用本规范的算法验证。

### 7.4 零上界与常数上界

- **零上界**：当某个槽位声明为 `0` 时，编译器直接检查推断代价是否恒为 0。
- **常数上界**：当某个槽位声明为常数 `c` 时，若推断代价也是常量则直接比较数值；当推断代价包含变量时，需要结合参数上界或退回更抽象的 `O(·)` 预算。

---

## 八、与递归分析的交互

### 8.1 递归分层回顾

根据 Part III (递归分析) 和 Part I (代价模型) 的设计，递归函数按可分析性分为三层：

| 层级 | 名称 | 代价分析方式 | 与本规范的交互 |
|------|------|-------------|---------------|
| Tier 1 | 结构递归 | 编译器自动生成 CostExpr | 生成的 CostExpr 由本算法验证 |
| Tier 2 | 声明式 | 开发者提供 CostExpr | 开发者提供的 CostExpr 由本算法验证 |
| Tier 3 | @unbounded | 无 CostExpr | 跳过验证 |

### 8.2 Tier 1：结构递归

编译器识别结构递归模式后，自动生成 CostExpr：

```
fn tree_sum(tree: Tree<I32>) -> I32 {
    match tree {
        Leaf(v) => v                                    -- cost: O(1)
        Node(left, v, right) => tree_sum(left) + v + tree_sum(right)
    }
}
```

编译器推断：
- 递归深度 ≤ depth(tree) = D
- 每层访问节点数求和 = N（总节点数）
- 每节点代价 = O(1)
- **生成的 CostExpr**：`N * 3 + 5`（常数因子由原语代价表确定）

此 CostExpr 由本规范的算法与签名声明的上界进行比较。

### 8.3 Tier 2：声明式

开发者提供代价声明，编译器验证：

```
fn merge_sort<T>(items: List<T, max: n>) -> List<T, max: n>
    where T: Ord
    cost [O(n log n), O(n), 0, 0]
{
    ...
}
```

编译器对函数体进行代价推断，生成内部 CostExpr C(n)，然后使用本规范的算法判定其是否落在声明的 `cost [O(n log n), O(n), 0, 0]` 四维预算内。

### 8.4 Tier 3：@unbounded

标记为 `unbounded` 的函数不参与代价验证，也不能被已声明 `cost [compute, alloc, io, parallel]` 的函数直接调用（除非通过运行时代价限制器包裹）。

---

## 九、边界情况与限制

### 9.1 纯常量表达式

当 C 和 B 都不含变量时，退化为数值比较：

```
C = 42, B = 100
验证：42 ≤ 100 → PASS
```

无需渐近分析，直接计算。

### 9.2 单变量情况

当只有一个变量 n 时，比较退化为标准的 O 记法比较。标准形的每个单项式为 `c × n^a × log(n)^b`，比较规则为：

```
(c₁, a₁, b₁) ≼ (c₂, a₂, b₂)  ⟺  a₁ < a₂
                                   ∨ (a₁ = a₂ ∧ b₁ < b₂)
                                   ∨ (a₁ = a₂ ∧ b₁ = b₂ ∧ c₁ ≤ c₂)
```

这与经典的渐近阶比较完全一致。

### 9.3 多变量情况

当变量数 k > 1 时，比较是逐变量的——使用向量指数的偏序（componentwise comparison）。这是一个 **偏序** 而非全序：两个单项式可能不可比较（如 `n*m` 和 `n²`）。

不可比较的情况导致算法返回 FAIL（保守），开发者需要调整声明使其可比较。

### 9.4 限制清单

| 限制 | 说明 | 应对策略 |
|------|------|---------|
| 无法表达条件代价 | 如"若已排序则 O(n)，否则 O(n²)" | 使用 `max(n, n^2) = n^2` 保守估计 |
| 无摊销分析 | 无法表达"均摊 O(1)"的操作 | 使用最坏情况代价 |
| 无概率分析 | 无法表达"期望 O(n log n)" | 使用最坏情况代价 |
| 不支持减法 / 除法 | 无法精确表达 `n*(n-1)/2` | 使用 `n^2` 上界 |
| max/min 嵌套深度有限 | 深度嵌套导致表达式膨胀 | 编译器限制嵌套深度（默认上限 8 层） |
| 多变量偏序不完全 | 不可比较的单项式导致 FAIL | 开发者调整声明或使用 max 包裹 |

---

## 十、示例

### 示例 1：常量代价（平凡验证）

```spore
fn get_first(pair: (I32, I32)) -> I32
    cost [100, 0, 0, 0]
{
    pair.0    -- 推断代价: 1 op (变量读取) = 1
}
```

**验证过程**：

```
C = 1,  B = 100
NF(C) = {(1, ∅, ∅)},  NF(B) = {(100, ∅, ∅)}
比较：(1, ∅, ∅) ≼ (100, ∅, ∅)?  指数相同(均为零向量), 系数 1 ≤ 100 ✓
→ PASS
```

### 示例 2：线性代价（系数比较）

```spore
fn sum_list(items: List<I32, max: n>) -> I32
    cost [n * 5 + 10, 0, 0, 0]
{
    items |> fold(start: 0, step: fn(acc, x) -> acc + x)
    -- 推断代价: n × (1+1+1) + 5 = 3*n + 5
}
```

**验证过程**：

```
C = 3*n + 5,  B = 5*n + 10
NF(C) = {(3, [1], [0]), (5, [0], [0])}
NF(B) = {(5, [1], [0]), (10, [0], [0])}

检查 (3, [1], [0]) ≼ (5, [1], [0])?  指数相同, 系数 3 ≤ 5 ✓
检查 (5, [0], [0]) ≼ (10, [0], [0])? 指数相同, 系数 5 ≤ 10 ✓
→ PASS
```

### 示例 3：N log N 代价（支配性检查）

```spore
fn merge_sort<T>(items: List<T, max: n>) -> List<T, max: n>
    where T: Ord
    cost [O(n log n), O(n), 0, 0]
{
    -- 推断代价: 2 * n * log(n) + n
}
```

**验证过程**：

```
C = 2*n*log(n) + n,  B = 3*n*log(n)
NF(C) = {(2, [1], [1]), (1, [1], [0])}
NF(B) = {(3, [1], [1])}

检查 (2, [1], [1]) ≼ (3, [1], [1])?  指数相同, 系数 2 ≤ 3 ✓
检查 (1, [1], [0]) ≼ (3, [1], [1])?  多项式指数 [1]=[1] 相等, 对数指数 [0] < [1] ✓ (严格支配)
→ PASS
```

### 示例 4：多变量代价

```spore
fn matrix_multiply(a: Matrix<n, m>, b: Matrix<m, k>) -> Matrix<n, k>
    cost [n * m * k, 0, 0, 0]
{
    -- 推断代价: n * m * k - n  (减法在实际推断中不出现；
    --   实际推断为 n*m*k 的某个下界，此处简化为 n*m*k - n 的等价上界)
    -- 保守推断代价: n * m * k (编译器不做减法，直接取上界)
}
```

**验证过程**（假设推断代价保守为 `n * m * k`）：

```
C = n*m*k,  B = n*m*k
NF(C) = {(1, [1,1,1], [0,0,0])}
NF(B) = {(1, [1,1,1], [0,0,0])}

检查 (1, [1,1,1], [0,0,0]) ≼ (1, [1,1,1], [0,0,0])?
  指数完全相同, 系数 1 ≤ 1 ✓
→ PASS
```

### 示例 5：验证失败

```spore
fn bad_search<T>(items: List<T, max: n>) -> Option<T>
    cost [n, 0, 0, 0]
{
    -- 实际实现了嵌套循环，推断代价: n * log(n)
}
```

**验证过程**：

```
C = n*log(n),  B = n
NF(C) = {(1, [1], [1])}
NF(B) = {(1, [1], [0])}

检查 (1, [1], [1]) ≼ (1, [1], [0])?
  多项式指数 [1] = [1] 相等, 对数指数 [1] > [0] ✗ — 不被支配!
→ FAIL
```

**编译器报错**：

```
ERROR [cost-exceeded] bad_search 的推断代价超出声明上界。
  推断代价: n * log(n)
  声明上界: n
  差异: log(n) 因子

  建议:
  (a) 将声明修改为 `cost [O(n log n), O(n), 0, 0]` 以匹配实际代价
  (b) 优化实现以消除 log(n) 因子
  (c) 检查是否有不必要的嵌套迭代
```

---

## 附录 A：符号约定

| 符号 | 含义 |
|------|------|
| ℕ | 非负整数集 {0, 1, 2, ...} |
| ℕ⁺ | 正整数集 {1, 2, 3, ...} |
| n̄ | 变量向量 (n₁, ..., nₖ) |
| ā, b̄ | 指数向量 |
| ⟦e⟧σ | 表达式 e 在赋值 σ 下的求值结果 |
| ≼ | 渐近支配关系 |
| NF(e) | 表达式 e 的标准形 |
| \|e\| | 表达式 e 的大小（AST 节点数） |
| ⌈x⌉ | 上取整 |
| O(·) | 渐近上界记法 |

---

# Part III: 递归代价分析 — 三层组合方案

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
  │  Tier 2: 声明式验证              ~20%     │  ← 开发者写 cost [compute, alloc, io, parallel]
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
按照 Part I（代价模型）中的原语代价表计算。

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
fn factorial(n: I32) -> I32 {
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
fn sum(list: List<I32>) -> I32 {
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
fn depth<T>(tree: Tree<T>) -> I32 {
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

在函数签名中通过 `cost [compute, alloc, io, parallel]` 子句声明代价上界：

```spore
fn ackermann(m: I32, n: I32) -> I32
    cost [ackermann_bound(m, n), 0, 0, 0]
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
fn gcd(a: I32, b: I32) -> I32
    decreases a + b
    cost [O(log n), 0, 0, 0]
{
    match b {
        0 => a,
        _ => gcd(b, a % b),
    }
}
```

`decreases expr` 语义：编译器验证 `expr` 在每次递归调用时严格递减且非负。

### 3.4 验证方法

编译器按以下顺序尝试验证 `cost [compute, alloc, io, parallel]` 各槽位：

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

如果编译器无法验证某个 `cost [compute, alloc, io, parallel]` 槽位：

```
WARNING [unverified-cost-bound] gcd 的代价上界无法自动验证。
  声明: cost [O(log n), 0, 0, 0]
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
    cost [O(n log n), O(n), 0, 0]
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

2. cost [O(n log n), O(n), 0, 0] where n = len(list):
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
fn collatz(n: I32) -> I32 {
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
| 上下文限制 | `@unbounded` 函数不能在已声明 `cost [compute, alloc, io, parallel]` 的上下文中直接使用 |
| Hole 交互 | `@unbounded` 函数内的 Hole 标记为 `cost_budget: unbounded` |

### 4.4 传染性与隔离

```spore
@unbounded
fn collatz(n: I32) -> I32 { ... }

// ✗ 编译错误: 不能在 cost-bounded 上下文中直接调用 @unbounded 函数
fn analyze(n: I32) -> I32
    cost [1000, 0, 0, 0]
{
    collatz(n)  // ERROR [unbounded-in-bounded-context]
}

// ✓ 通过代价限制器包裹
fn safe_analyze(n: I32) -> I32 ! [CostExceeded]
    cost [1000, 0, 0, 0]
{
    with_cost_limit(1000) {
        collatz(n)
    }
}

// ✓ 调用者也标记为 @unbounded
@unbounded
fn analyze_all(ns: List<I32>) -> List<I32> {
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
fn matrix_sum(matrix: List<List<I32>>) -> I32 {
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
fn is_even(n: I32) -> Bool {
    match n {
        0 => true,
        n => is_odd(n - 1),
    }
}

fn is_odd(n: I32) -> Bool {
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
| SCC 不满足结构递归 | SCC 中**所有**函数需要 `cost [...]` 或 `@unbounded` |
| SCC 中部分函数有 `@unbounded` | 整个 SCC 视为 `@unbounded` |

---

## 七、与 Hole 系统的交互

### 7.1 递归函数中的 Hole

当 Hole 出现在递归函数内部时，HoleReport 需要考虑递归带来的代价开销：

```spore
fn process_tree<T>(tree: Tree<T>) -> Result<T>
    cost [500, 0, 0, 0]
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
fn factorial(n: I32) -> I32 {
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
fn tree_sum(tree: Tree<I32>) -> I32 {
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
fn binary_search<T>(sorted: List<T>, target: T) -> Option<I32>
    where T: Ord
{
    search_helper(sorted, target, 0, len(sorted) - 1)
}

fn search_helper<T>(sorted: List<T>, target: T, low: I32, high: I32) -> Option<I32>
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
    cost [O(n log n), O(n), 0, 0]
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

  2. cost [O(n log n), O(n), 0, 0]:
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
fn collatz_steps(n: I32) -> I32 {
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
   是否支持 `cost [O(n log n) expected, 0, 0, 0]` 这类带期望语义的声明？
   留待后续版本设计。
