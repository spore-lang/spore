# Spore 能力/效果代数 — 设计文档 v0.1

> **Status**: Draft
> **Scope**: `sporec` 编译器能力检查、函数类型编码、性质推导、与 Hole 系统及代价模型的交互
> **Depends on**: Signature syntax v0.2, Hole system v0.2, Cost model v0.1, Type system v0.1

---

## 1. 原子能力集 (Atomic Capability Set)

### 1.1 定义

设 **E** 为原子能力的全集（universe of atomic capabilities）。每个原子能力是一个不可再分的标识符，
代表程序可能与外部世界交互的一种方式。

### 1.2 内置原子能力

Spore 提供以下内置原子能力：

| 能力 | 语义 | 典型操作 |
|------|------|---------|
| `FileRead` | 读取文件系统 | `read_file`, `list_dir` |
| `FileWrite` | 写入文件系统 | `write_file`, `create_dir`, `delete` |
| `NetRead` | 网络读取 | `http.get`, `tcp.recv` |
| `NetWrite` | 网络写入 | `http.post`, `tcp.send` |
| `StateRead` | 读取可变状态 | `state.get`, `cache.lookup` |
| `StateWrite` | 写入可变状态 | `state.set`, `cache.insert` |
| `Spawn` | 创建并发任务 | `spawn { ... }` |
| `Clock` | 读取系统时钟 | `now()`, `elapsed()` |
| `Random` | 生成随机数 | `random()`, `uuid()` |
| `Compute` | 纯计算（非平凡） | 编译器隐式标注 |
| `Exit` | 终止进程 | `exit()`, `abort()` |

### 1.3 能力集

**能力集** S 是 E 的有限子集：

$$S \subseteq E, \quad |S| < \infty$$

空集 `{}` 表示纯函数——不与外部世界交互。

### 1.4 `capability` 关键字：命名别名

`capability` 关键字创建一个 **命名别名**，展开后得到原子能力的扁平集合：

```spore
capability FileIO = [FileRead, FileWrite]
capability DatabaseAccess = [NetRead, NetWrite, StateRead, StateWrite]
capability FullIO = [FileIO, NetRead, NetWrite]
```

别名展开是 **完全展开**（递归扁平化）：

```
FullIO → {FileIO, NetRead, NetWrite}
       → {FileRead, FileWrite, NetRead, NetWrite}
```

> **关键决策**: 没有"能力变量"（no effect variables）。所有能力集在编译期必须完全确定。
> 不存在 `uses E` 泛型参数。

---

## 2. 集合代数 (Set Algebra)

能力集遵循标准有限集合代数的性质。

### 2.1 基本性质

| 性质 | 公式 | 说明 |
|------|------|------|
| **交换律** | {A, B} = {B, A} | 能力声明顺序无关 |
| **幂等律** | {A, A} = {A} | 重复声明无效果 |
| **结合律** | 嵌套别名扁平化 | `[FileIO, NetRead]` = `[FileRead, FileWrite, NetRead]` |
| **单位元** | {} （空集） | 空集 = 纯函数，零效果 |

### 2.2 别名展开规则

给定别名定义：

$$\text{capability } C = [A_1, A_2, \ldots, A_n]$$

则在任何 `uses` 声明中：

$$\text{uses } [C] \equiv \text{uses } [A_1, A_2, \ldots, A_n]$$

展开是递归的。若 $A_i$ 本身是别名，则继续展开直到所有元素为原子能力。

### 2.3 集合运算

| 运算 | 符号 | 语义 |
|------|------|------|
| 并集 | S₁ ∪ S₂ | 合并两个能力集（顺序组合、条件分支） |
| 子集 | S₁ ⊆ S₂ | S₁ 是 S₂ 的子集（子类型判断基础） |
| 交集 | S₁ ∩ S₂ | 共有能力（用于性质推导） |
| 差集 | S₁ \ S₂ | 在 S₁ 中但不在 S₂ 中（保留，见开放问题） |

---

## 3. 子类型关系与能力缩窄 (Subtyping & Capability Narrowing)

### 3.1 子类型规则

能力集之间的子类型关系基于集合包含：

$$S_1 \subseteq S_2 \implies (\tau \to \rho \ \textbf{uses}\ S_1) <: (\tau \to \rho \ \textbf{uses}\ S_2)$$

即：**需要更少能力的函数可以被用在需要更多能力的位置**。函数类型在能力参数上是**逆变**的（contravariant）——
使用更小能力集的函数更"通用"。

直觉：纯函数（`uses {}`）是所有函数类型的子类型，可以在任何上下文中调用。

### 3.2 核心推导规则

```
                    S₁ ⊆ S₂
─────────────────────────────────────────  [CAP-SUB]
(T → R uses S₁) <: (T → R uses S₂)
```

```
─────────────────────────────────────────  [CAP-PURE]
(T → R uses {}) <: (T → R uses S)     ∀ S
```

### 3.3 能力缩窄 (Capability Narrowing)

在 `parallel_scope` 中，子任务的能力集必须是父任务能力集的子集：

$$S_{\text{child}} \subseteq S_{\text{parent}}$$

这保证子任务不会获得超出父任务授权范围的能力。

```spore
fn process(data: List[Item]) -> List[Result] ! [NetworkError]
uses [NetRead, Spawn]
{
    parallel_scope {
        -- 子任务只能使用 NetRead（⊆ {NetRead, Spawn}）
        let tasks = data.iter().map(|item| spawn {
            fetch(item)  -- fetch uses [NetRead]
        })
        tasks.map(|task| task.await)
    }
}
```

### 3.4 类型判断 (Typing Judgment)

标准判断形式：

$$\Gamma;\ S \vdash e : T$$

读作："在类型上下文 Γ 和能力集 S 下，表达式 e 的类型为 T"。

核心规则：

```
Γ; S ⊢ e₁ : T₁    Γ; S ⊢ e₂ : T₂
────────────────────────────────────────  [SEQ]
Γ; S ⊢ (e₁; e₂) : T₂


Γ; S ⊢ f : (T → R uses S_f)    Γ; S ⊢ x : T    S_f ⊆ S
───────────────────────────────────────────────────────────  [APP]
Γ; S ⊢ f(x) : R


────────────────────────────────  [PURE-LITERAL]
Γ; S ⊢ 42 : I32      ∀ S
```

---

## 4. 函数类型与能力编码 (Function Types)

### 4.1 完整函数类型

函数类型的完整形式：

```
(T₁, T₂, ..., Tₙ) -> R  uses S  ! [E₁, E₂, ...]
```

其中：
- `T₁, T₂, ..., Tₙ` — 参数类型
- `R` — 返回类型
- `S` — 能力集
- `[E₁, E₂, ...]` — 错误类型集

### 4.2 简写形式

当能力集为空（纯函数）时，`uses` 子句可省略：

```spore
-- 完整形式
fn add(a: I32, b: I32) -> I32 uses [] { a + b }

-- 简写形式（等价）
fn add(a: I32, b: I32) -> I32 { a + b }
```

两种写法在类型系统中完全等价：

$$(T_1, T_2) \to R \equiv (T_1, T_2) \to R \ \textbf{uses}\ \{\}$$

### 4.3 高阶函数：纯闭包约束

高阶函数（如 `map`, `fold`, `filter`）**只接受纯闭包**：

```spore
fn map[T, U](list: List[T], f: (T) -> U) -> List[U] {
    ...
}
```

此处 `f: (T) -> U` 没有 `uses` 子句，意味着 `f` 必须是纯函数（`uses {}`）。

> **关键决策**: 高阶函数不接受有副作用的闭包。需要在迭代中执行副作用操作时，
> 使用 `parallel_scope` + `spawn` 模式。

### 4.4 闭包能力捕获

闭包在定义时捕获其所在上下文的能力集。闭包的能力集 S' 是上下文能力集 S 的子集：

$$\text{closure } |x| \text{ expr} \text{ 在上下文 } S \text{ 中} \implies \text{closure type: } (T) \to U \ \textbf{uses}\ S' \text{ where } S' \subseteq S$$

S' 由闭包体内实际使用的能力决定（编译器推断）。

```spore
fn example() -> ()
uses [FileRead, NetRead]
{
    -- 此闭包类型为 (Str) -> Data uses [NetRead]
    let fetch_fn = |url| http.get(url)

    -- 此闭包类型为 (I32) -> I32 uses []  （纯函数）
    let double = |x| x * 2
}
```

---

## 5. 性质自动推导 (Property Auto-Inference)

### 5.1 推导规则

编译器根据 `uses` 声明 **自动推断** 函数性质，无需手动标注。`with` 子句已移除。

| 条件 | 推断结果 |
|------|---------|
| `uses {}` | pure ∧ deterministic ∧ total |
| `uses {Compute}` | deterministic（无 IO、无状态、无随机） |
| `uses S` 且 S ∩ {Clock, Random} ≠ ∅ | ¬deterministic |
| `uses S` 且 S ∩ {FileRead, FileWrite, NetRead, NetWrite, StateRead, StateWrite} ≠ ∅ | ¬pure |

### 5.2 形式化

定义性质推导函数 𝒫：

$$\mathcal{P}(\text{pure}, S) = \begin{cases} \text{true} & \text{if } S = \emptyset \\ \text{false} & \text{if } S \cap \{FileRead, FileWrite, NetRead, NetWrite, StateRead, StateWrite\} \neq \emptyset \end{cases}$$

$$\mathcal{P}(\text{deterministic}, S) = \begin{cases} \text{true} & \text{if } S \cap \{Clock, Random\} = \emptyset \\ \text{false} & \text{otherwise} \end{cases}$$

$$\mathcal{P}(\text{total}, S) = \text{由终止性分析独立判断（见 5.4）}$$

### 5.3 蕴含关系

性质之间存在蕴含链：

$$\text{pure} \implies \text{deterministic}$$

即：纯函数必然是确定性的（因为 `uses {}` 自然不含 Clock 和 Random）。

### 5.4 无法静态推断的性质

**幂等性** (idempotent): 一般情况下无法从能力集静态推断。使用文档注释标注：

```spore
/// @idempotent
fn sync_user(user_id: UserId) -> SyncResult ! [NetworkError]
uses [NetRead, NetWrite, StateRead, StateWrite]
{
    ...
}
```

**全性** (total): 对于结构递归可自动推断终止性；其他情况需要证明或 `@unbounded` 标注：

```spore
/// @unbounded
fn event_loop() -> Never
uses [NetRead, NetWrite]
{
    loop { handle_next_event() }
}
```

---

## 6. 能力合成规则 (Capability Composition Rules)

当多个表达式组合时，编译器按以下规则计算合成能力集。

### 6.1 顺序组合 (Sequential)

```
Γ; S ⊢ A : T₁    Γ; S ⊢ B : T₂
──────────────────────────────────  [SEQ-CAP]
capabilities(A; B) = S_A ∪ S_B
```

即：顺序执行两个表达式，总能力集是两者的并集。

### 6.2 条件分支 (Conditional)

```
Γ; S ⊢ c : Bool    Γ; S ⊢ A : T    Γ; S ⊢ B : T
────────────────────────────────────────────────────  [COND-CAP]
capabilities(if c then A else B) = S_c ∪ S_A ∪ S_B
```

条件表达式的能力集包括条件本身和所有分支的并集（保守上界）。

### 6.3 函数调用 (Function Call)

```
f : (T → R uses S_f)    S_f ⊆ S_scope
──────────────────────────────────────  [CALL-CAP]
calling f in scope S_scope is allowed
```

调用函数 f 要求当前作用域的能力集覆盖 f 的能力需求。

### 6.4 spawn 表达式

```
Spawn ∈ S_scope    S_body ⊆ S_scope
──────────────────────────────────────  [SPAWN-CAP]
Γ; S_scope ⊢ spawn { body } : Task[T]
```

`spawn` 有两个要求：
1. 当前作用域必须持有 `Spawn` 能力
2. 子任务体的能力需求必须是当前作用域的子集

`spawn` 表达式本身的类型是 `Task[T]`，其结果是纯的（创建任务句柄不需要副作用）。

### 6.5 合成规则总览

| 组合形式 | 能力集计算 |
|---------|-----------|
| `A; B` | S_A ∪ S_B |
| `if c then A else B` | S_c ∪ S_A ∪ S_B |
| `match x { p₁ => A, p₂ => B, ... }` | S_x ∪ S_A ∪ S_B ∪ ... |
| `f(x)` 其中 f uses S_f | 要求 S_f ⊆ S_scope |
| `spawn { body }` 其中 body uses S_b | 要求 Spawn ∈ S_scope 且 S_b ⊆ S_scope |
| `let x = e₁ in e₂` | S_e₁ ∪ S_e₂ |

---

## 7. 与 Hole 系统的交互 (Interaction with Hole System)

### 7.1 Hole 上下文中的能力信息

当编译器遇到 Hole（`?name`）时，生成的 `HoleReport` 包含当前位置可用的能力集：

```json
{
  "hole": "fetch_logic",
  "expected_type": "Data",
  "bindings": {
    "url": "Url",
    "timeout": "Duration"
  },
  "available_capabilities": ["NetRead"],
  "cost_budget": 5000,
  "candidate_functions": [
    "http.get(url: Url) -> Data ! [NetworkError] uses [NetRead]"
  ]
}
```

### 7.2 填充约束

Agent 或开发者填充 Hole 时，填入代码的能力需求必须不超过可用能力集：

$$S_{\text{fill}} \subseteq S_{\text{available}}$$

编译器在 Hole 填充后验证此约束。若违反，报错：

```
ERROR [cap-violation] Hole ?fetch_logic 填充代码使用了未授权的能力:
  可用能力: [NetRead]
  填入代码需要: [NetRead, FileWrite]
  超出能力: [FileWrite]
```

### 7.3 能力集对候选函数过滤的影响

Hole 系统在列出候选函数时，会根据 `available_capabilities` 过滤：
只有 `uses S` 满足 S ⊆ S_available 的函数才会出现在候选列表中。

---

## 8. 与代价模型的交互 (Interaction with Cost Model)

### 8.1 能力集与代价维度的映射

能力集直接影响代价模型的四维向量 `(compute, alloc, io, parallel)`：

| 条件 | 代价维度约束 |
|------|-------------|
| `uses {}` | io = 0（保证无 IO 开销） |
| `uses S` 且 S ∩ {NetRead, NetWrite, FileRead, FileWrite} ≠ ∅ | io > 0 可能 |
| `Spawn ∈ S` | parallel (lane) > 0 可能 |
| `uses {Compute}` | 仅 compute 和 alloc 维度非零 |

### 8.2 能力与代价的关系

能力集是代价推导的 **必要条件**（necessary condition），但非充分条件：

- `uses [NetRead]` 不意味着一定有 IO 开销（函数可能在某些路径上不做网络调用）
- 代价模型进行更精细的抽象解释，能力集提供上界约束

$$S \cap \{NetRead, NetWrite, FileRead, FileWrite\} = \emptyset \implies \text{cost}_{\text{io}} = 0$$

这是 **硬保证**：编译器可以依赖此关系进行优化。

---

## 9. 示例 (Examples)

### 示例 1：纯函数 (uses [])

```spore
fn fibonacci(n: I32) -> I32 {
    match n {
        0 => 0,
        1 => 1,
        n => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
```

编译器推断：
```
uses []
-- 自动推断: pure, deterministic
-- total: 需要终止性证明（结构递归，n 递减 → 可证明）
cost [O(2^n), 0, 0, 0]
```

### 示例 2：IO 函数 (uses [FileRead])

```spore
fn read_config(path: Str) -> Config ! [IoError, ParseError]
uses [FileRead]
{
    let content = read_file(path)
    parse_toml(content)
}
```

编译器推断：
```
uses [FileRead]
-- 自动推断: ¬pure, deterministic（FileRead 不破坏确定性）
-- total: 是（无递归）
cost [200, 0, 1, 0]
```

### 示例 3：能力别名 (capability alias)

```spore
capability DatabaseAccess = [NetRead, NetWrite, StateRead, StateWrite]

fn query_user(id: UserId) -> User ! [DbError, NotFound]
uses [DatabaseAccess]
{
    let conn = pool.get_connection()
    conn.query("SELECT * FROM users WHERE id = ?", id)
}
```

展开后等价于：
```
uses [NetRead, NetWrite, StateRead, StateWrite]
-- 自动推断: ¬pure, deterministic
```

### 示例 4：高阶函数与纯闭包

```spore
fn process_scores(scores: List[I32]) -> List[Str] {
    scores
        .filter(|s| s >= 60)          -- 纯闭包: (I32) -> Bool
        .map(|s| format("Pass: {}", s))  -- 纯闭包: (I32) -> Str
}
```

`filter` 和 `map` 的参数类型要求闭包为纯函数。以下代码 **不合法**：

```spore
fn bad_example(scores: List[I32]) -> List[()]
uses [FileWrite]
{
    -- ❌ 编译错误: map 要求纯闭包，但此闭包 uses [FileWrite]
    scores.map(|s| write_file("log.txt", s.to_string()))
}
```

### 示例 5：parallel_scope + spawn 模式

对于需要在迭代中执行副作用的场景，使用 `parallel_scope` + `spawn`：

```spore
fn fetch_all(urls: List[Url]) -> List[Response] ! [NetworkError]
uses [NetRead, Spawn]
{
    parallel_scope {
        let tasks = urls.map(|url| {
            spawn {
                -- spawn 体内 uses [NetRead]，⊆ 父作用域 {NetRead, Spawn}  ✓
                http.get(url)
            }
        })
        tasks.map(|task| task.await)
    }
}
```

能力检查过程：
1. `fetch_all` 声明 `uses [NetRead, Spawn]`
2. `spawn { ... }` 要求 `Spawn ∈ {NetRead, Spawn}` ✓
3. `spawn` 体内调用 `http.get` 需要 `NetRead`，`{NetRead} ⊆ {NetRead, Spawn}` ✓
4. `urls.map(|url| spawn { ... })` — 传给 `map` 的闭包返回 `Task[Response]`，
   `spawn` 表达式本身是纯的（返回任务句柄），所以闭包满足 `uses {}` 要求 ✓

---

## 10. 开放问题 (Open Questions)

### 10.1 能力差集 (Capability Subtraction)

是否需要支持"排除某些能力"的语法？

```spore
-- 假设语法（尚未确定）
uses [All \ Spawn]  -- "除 Spawn 外的所有能力"
```

**顾虑**: 这引入了对 E（全集）的依赖，而 E 可能随平台扩展。目前倾向于 **不支持**，
要求显式列出所需能力。

### 10.2 平台能力天花板 (Platform Capability Ceiling)

平台系统（platform-system-v0.1）定义了模块级能力上限。两者的交互需要明确：

- 模块声明 `platform [Web]` 是否自动限制该模块所有函数的最大能力集？
- 平台能力天花板与函数级 `uses` 声明的关系：是交集还是约束检查？

### 10.3 能力变量 (Effect Variables — 未来可能)

当前设计明确排除能力变量。但未来如果出现强需求（如高度泛型的中间件库），
可考虑作为 **高级 opt-in 特性** 引入：

```spore
-- 假设的未来语法（当前不支持）
fn with_timeout[E](duration: Duration, f: () -> T uses E) -> T ! [Timeout]
uses [E, Clock]
{
    ...
}
```

引入前需评估对类型推断复杂度和编译器实现的影响。

---

## 附录 A: 形式化记法汇总

| 记法 | 含义 |
|------|------|
| E | 原子能力全集 |
| S, S₁, S₂ | 能力集（E 的有限子集） |
| {} 或 ∅ | 空能力集（纯函数） |
| S₁ ⊆ S₂ | S₁ 是 S₂ 的子集 |
| S₁ ∪ S₂ | S₁ 与 S₂ 的并集 |
| S₁ ∩ S₂ | S₁ 与 S₂ 的交集 |
| (T → R uses S) | 函数类型，参数 T，返回 R，能力集 S |
| Γ; S ⊢ e : T | 类型判断：上下文 Γ、能力集 S 下 e 有类型 T |
| 𝒫(prop, S) | 性质推导函数：根据能力集 S 判断性质 prop |
| <: | 子类型关系 |

## 附录 B: 设计决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 能力模型 | 扁平原子集合 | 简单、可组合、无层级复杂度 |
| 能力变量 | 不支持 | 降低类型推断复杂度；所有能力集编译期确定 |
| 函数能力编码 | `uses` 子句 | 与类型签名自然融合，纯函数零语法开销 |
| 高阶函数闭包 | 仅纯闭包 | 保证 `map`/`filter` 等组合子的可预测性和安全性 |
| `with` 子句 | 移除 | 性质由编译器从 `uses` 自动推断，减少冗余标注 |
| 能力差集 | 暂不支持 | 依赖全集定义，与平台扩展冲突 |
