# 模拟执行与抽象代价模型 — 完整设计 v0.1

## 一、核心理念

传统语言的性能分析依赖：运行时 profiling → 机器相关 → 不可复现。
我们的方案：**编译时模拟执行** → 机器无关 → 确定性可复现。

核心等式：
```
模拟执行 = 抽象解释(Abstract Interpretation) + 确定性代价表
```

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

**签名层面**：C, A, W 三个维度折算为单一标量 `cost`（加权求和）；`P` 作为独立维度报告，默认不参与标量求和。
**查询层面**：`sporec --query-cost fn_name` 返回完整四维明细 + 分解。

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
fn sum_list(items: List[Int]) -> Int {
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

签名中的 `cost ≤ K` 约束此时意味着：
- 编译器验证 `N × 2 + 5 ≤ K` 在给定 N 范围内成立
- 如果参数类型带有大小约束（如 `List[Int, max: 1000]`），编译器可以验证
- 如果无大小约束，编译器要求开发者明确声明

### 5.2 有界类型（Bounded Types）

为了让代价系统在编译时可验证，引入大小约束类型：

```
fn process_batch(items: List[Order, max: 500]) -> BatchResult ! [TooLarge]
    cost ≤ 25000
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
fn tree_depth[T](tree: Tree[T]) -> Int {
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
fn fibonacci(n: Int) -> Int {
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
  (a) 添加 `cost ≤ K` + 参数约束 `n: Int if n ≤ 30`
  (b) 标记为 `unbounded`（放弃代价约束）
  (c) 改用结构递归或尾递归 + 迭代上限
```

标记为 `unbounded` 的函数：
```
fn fibonacci(n: Int) -> Int
    cost: unbounded
{
    ...
}
```

`unbounded` 函数**不能被 cost ≤ K 函数调用**，除非包裹在运行时代价限制器中：
```
fn safe_fib(n: Int) -> Int ! [CostExceeded]
    cost ≤ 10000
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
    cost ≤ N × log(N) × 3 + N
    uses [Compute]
{
    ...
}
```

符号代价支持的操作符：
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
    ├── 与签名中 `cost ≤ K` 比对
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
  "cost_declared": "≤ 200",
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
  "cost_symbolic": "N × log(N) × 3 + N",
  "cost_declared": "≤ N × log(N) × 3 + N",
  "variables": { "N": "len(items), max = param constraint" },
  "status": "symbolic_match"
}
```

---

## 八、与 Hole 系统的交互

部分定义的函数也参与代价分析：

```
fn process(data: Data) -> Result ! [ProcessError]
    cost ≤ 1000
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
         cost ≤ N × cost(f) + N
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
       cost ≤ 500
       uses [Compute]
   ```
   - 如果外部函数未声明代价，则视为 `unbounded`，与未标注代价的一般递归函数受相同限制：不能被 `cost ≤ K` 函数直接调用。
   - 这确保了 FFI 边界的代价透明性——外部世界不能静默地引入代价黑洞。

4. **代价漂移检测**（Future Work）：实际运行时代价与编译时预测的偏差监控。
   - 此问题可通过测试/验证系统部分解决：运行时对代价进行采样，与编译时预测值进行对比。
   - 可配置容差阈值（如 `cost_drift_tolerance = 1.2` 表示允许 20% 偏差），超出阈值时报告警告。
   - 留待后续版本设计完整的运行时代价采样框架。
