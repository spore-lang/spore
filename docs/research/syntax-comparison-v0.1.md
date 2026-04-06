# 函数签名语法方案对比（含参考语言分析）

> 📦 **Frozen reference material.** This comparative analysis informed the final syntax design in syntax-spec-v0.1.md §5 and is preserved as-is for historical context.

## 一、参考语言的签名语法

### 1. Koka — 效果行（Effect Row）嵌入返回箭头

```koka
// 纯函数
fun add(x: int, y: int) : total int

// 单效果
fun printLine(msg: string) : console ()

// 多效果
fun fetchAndLog(url: string) : <net, console, exn> string

// 效果多态
fun map(xs: list<a>, f: (a) -> <e> b) : <e> list<b>
```

**特点：** 效果写在 `:` 和返回类型之间，用 `<>` 包裹。纯函数标 `total`（通常省略）。
效果是 *类型系统的一部分*，编译器自动推断。

---

### 2. Roc — `!` sigil + Task 返回类型

```roc
# 纯函数 — 无 sigil
add : Num, Num -> Num
add = \x, y -> x + y

# 有效函数 — ! 后缀
printLine! : Str -> Task {} [StdoutErr]
readFile! : Str -> Task Str [FileReadErr, NotFound]

# 错误在 Task 的第二个参数
main! = |_args|
    content = readFile!("data.txt")?    # ? 传播错误
    printLine!(content)?
    Ok({})
```

**特点：** 纯/不纯在 *函数名* 上一眼可见（`!` sigil）。
错误类型嵌在 `Task ok err` 里。无单独的效果标注——只有"纯"和"不纯"两档。

---

### 3. Elm — Haskell 风格类型签名 + Result

```elm
-- 纯函数
add : Int -> Int -> Int
add x y = x + y

-- 错误处理用 Result
safeDivide : Float -> Float -> Result MathError Float

-- 副作用通过 Cmd/Sub 建模，不在函数签名里体现
update : Msg -> Model -> ( Model, Cmd Msg )
```

**特点：** 签名极简——只有参数类型和返回类型。
没有效果标注（Elm 通过架构约束效果，不通过类型系统）。
错误用 `Result err ok` 显式表达。

---

### 4. Unison — 花括号能力声明（Ability）

```unison
-- 纯函数（无花括号）
add : Nat -> Nat -> Nat

-- 单能力
printValue : Text ->{IO} ()

-- 多能力
saveOrAbort : Data ->{IO, Abort} Boolean

-- 能力多态
map : (a ->{e} b) -> [a] ->{e} [b]

-- 自定义能力
ability Logger where
  log : Text ->{Logger} ()

-- 能力处理器（去除能力）
catch : '{g, Exception} a ->{g} Either Failure a
```

**特点：** 能力写在箭头 `->` 和返回类型之间，`{}` 包裹。
空 `{}` 表示纯。自定义能力通过 `ability ... where` 定义。
与 Koka 类似但用花括号而非尖括号。

---

### 5. Idris 2 — totality 注解 + dependent effects

```idris
-- 纯函数，全函数
total
add : Int -> Int -> Int
add x y = x + y

-- 偏函数
partial
unsafeHead : List a -> a
unsafeHead (x::xs) = x

-- 依赖类型
append : Vect n a -> Vect m a -> Vect (n + m) a

-- IO 效果
main : IO ()
main = putStrLn "Hello"

-- 依赖效果（效果集随值变化）
readInt : Eff Bool [STATE (Vect n Int), STDIO]
   (\ok => if ok
           then [STATE (Vect (S n) Int), STDIO]
           else [STATE (Vect n Int), STDIO])
```

**特点：** totality（`total`/`partial`/`covering`）作为 *独立注解* 放在签名之上。
类型签名和函数定义分开写。效果用 `Eff` 类型表达，可依赖值。

---

### 6. Agda — 类型签名 + where + typed holes

```agda
-- 纯函数
add : ℕ → ℕ → ℕ
add x y = x + y

-- 带 where 块（局部定义）
quadruple : ℕ → ℕ
quadruple n = double (double n)
  where
    double : ℕ → ℕ
    double zero    = zero
    double (suc n) = suc (suc (double n))

-- Typed hole
process : List ℕ → ℕ
process xs = ?    -- 编译器告诉你此处需要 ℕ，可用 xs : List ℕ
```

**特点：** 签名和定义完全分离。`where` 仅用于局部辅助定义。
Hole 用 `?` 标记，编译器交互式给出上下文。无效果系统——纯语言。

---

### 7. Effekt — 能力作为参数传递

```effekt
// 效果声明
effect Yield(n: Int): Bool

// 能力作为参数
def counter(y: Yield): Unit = {
  var n = 0
  var produce = true
  while (produce) {
    produce = do yield(n)
    n = n + 1
  }
}

// 多能力
def drunkFlip(amb: Amb, exc: Exc): String = { ... }
```

**特点：** 能力是 *显式参数*，不是类型注解。
调用者必须提供能力实例。最接近 OOP 的依赖注入思维。

---

## 二、各语言方法总结

| 语言    | 效果表达位置        | 错误表达方式          | 纯度标记       | 额外元数据     |
|---------|--------------------|-----------------------|---------------|---------------|
| Koka    | 返回箭头后 `<e>`    | `exn` 效果            | `total` 关键字 | 无            |
| Roc     | 函数名 `!` sigil    | `Task ok [errs]`      | 无 `!` = 纯    | 无            |
| Elm     | 不在签名中          | `Result err ok`       | 全部是纯       | 无            |
| Unison  | 返回箭头后 `{}`     | 自定义 ability        | 空 `{}` = 纯   | 内容寻址 hash |
| Idris   | 独立行 / Eff 类型   | 类型层面              | `total` 注解   | totality 检查 |
| Agda    | 无（纯语言）        | 依赖类型              | 全部是纯       | where 局部定义 |
| Effekt  | 能力作为函数参数     | 能力参数              | 无能力 = 纯    | 无            |

---

## 三、我们的三个语法方案（含详细示例）

### 设计目标回顾
签名需承载：函数名、具名参数、返回类型、错误类型、效果标注、
代价上界、能力集（uses）。所有这些都参与 snapshot hash。

---

### 方案 A：管道式（Pipe-delimited）

**灵感来源：** Koka 的独立行效果 + Roc 的清晰结构 + 自定义扩展

```
-- 简单纯函数（自由函数，编译器推断所有元数据）
fn add(a: Int, b: Int) -> Int {
    a + b
}

-- 中等复杂度
fn parse_config(raw: String, strict: Bool) -> Config
  | errors [MalformedInput, MissingField]
  | pure deterministic
  | cost ≤ 200
  | uses [Compute, Module<toml>]
{
    ...
}

-- 完整复杂函数
fn sync_user_data(user_id: UserId, source: DataSource) -> SyncReport
  | errors [NetworkTimeout, AuthExpired, DataConflict]
  | idempotent
  | cost ≤ 8500
  | uses [NetRead, NetWrite, StateRead, Module<auth>, FuncCall<merge_records>]
{
    ...
}

-- Hole（部分定义）
fn validate_payment(amount: Money, method: PaymentMethod) -> Receipt
  | errors [Declined, InsufficientFunds]
  | idempotent
  | uses [NetRead, Module<payment_gateway>]
{
    ?validate_logic    -- typed hole，模拟执行时输出上下文
}

-- 自定义能力
capability DatabaseAccess = [NetRead, NetWrite, StateRead, StateWrite]

fn query_users(filter: Filter) -> List<User>
  | errors [ConnectionLost, QueryTimeout]
  | deterministic
  | cost ≤ 3000
  | uses [DatabaseAccess, Compute]
{
    ...
}
```

**优点：**
- 每行一个维度，视觉对齐，便于 diff
- `|` 前缀让元数据与参数列表视觉区分明显
- Agent 解析简单：逐行 parse，每行 `| keyword value`
- 增加新维度只需新增一行，向前兼容

**缺点：**
- 复杂函数签名会占很多行
- `|` 在某些语言里有其他含义（union type、pattern match）

---

### 方案 B：Where 块式

**灵感来源：** Haskell/Rust 的 where 约束 + Agda 的局部定义块

```
-- 简单纯函数
fn add(a: Int, b: Int) -> Int {
    a + b
}

-- 中等复杂度
fn parse_config(raw: String, strict: Bool) -> Config
where
    errors: [MalformedInput, MissingField]
with [pure, deterministic]
cost ≤ 200
uses [Compute, Module<toml>]
{
    ...
}

-- 完整复杂函数
fn sync_user_data(user_id: UserId, source: DataSource) -> SyncReport
where
    errors: [NetworkTimeout, AuthExpired, DataConflict]
with [idempotent]
cost ≤ 8500
uses [NetRead, NetWrite, StateRead, Module<auth>, FuncCall<merge_records>]
{
    ...
}

-- Hole
fn validate_payment(amount: Money, method: PaymentMethod) -> Receipt
where
    errors: [Declined, InsufficientFunds]
with [idempotent]
uses [NetRead, Module<payment_gateway>]
{
    ?validate_logic
}
```

**优点：**
- 效果用 `with [...]`、代价用 `cost ≤ N`、能力用 `uses [...]`，各自独立一行
- `where` 仅保留给类型约束和错误声明，语义更清晰
- 熟悉 Haskell/Rust 的开发者对 `where` 约束直觉上理解

**缺点：**
- 复杂函数签名仍可能占多行
- 多种关键字（`where`/`with`/`cost`/`uses`）需要记忆
- 纯函数和复杂函数之间的结构差异较大

---

### 方案 C：混合式（Sigil + Where）

**灵感来源：** Roc 的 `!` sigil + Koka 的内联效果 + Where 补充

```
-- 纯函数（无 sigil，无 where）
fn add(a: Int, b: Int) -> Int {
    a + b
}

-- 有效函数（! sigil + 错误在返回类型后）
fn parse_config!(raw: String, strict: Bool) -> Config ![MalformedInput, MissingField]
cost ≤ 200
uses [Compute, Module<toml>]
{
    ...
}

-- 纯但有错误的函数（无 ! 但有 !错误）
fn validate(input: String) -> Bool ![ValidationError]
cost ≤ 50
{
    ...
}

-- 完整复杂函数
fn sync_user_data!(user_id: UserId, source: DataSource) -> SyncReport ![NetworkTimeout, AuthExpired, DataConflict]
with [idempotent]
cost ≤ 8500
uses [NetRead, NetWrite, StateRead, Module<auth>, FuncCall<merge_records>]
{
    ...
}

-- Hole
fn validate_payment!(amount: Money, method: PaymentMethod) -> Receipt ![Declined, InsufficientFunds]
with [idempotent]
uses [NetRead, Module<payment_gateway>]
{
    ?validate_logic
}
```

**优点：**
- `!` 在函数名上一眼可见纯/不纯（Roc 验证过的 UX）
- 错误类型 `![...]` 紧跟返回类型，是签名最核心的信息
- 效果用 `with [...]`、代价用 `cost ≤ N`、能力用 `uses [...]`，无需 where 包裹
- 信息密度最高——最重要的信息在最显眼的位置

**缺点：**
- `!` 的语义要精确定义（仅表示有能力依赖？还是有 IO？）
- 错误类型用 `!` 前缀可能与其他语法冲突
- 一行可能很长（函数名 + 参数 + 返回类型 + 错误类型）

---

## 四、对比矩阵

| 维度              | 方案A 管道式    | 方案B Where块   | 方案C 混合式     |
|-------------------|----------------|----------------|-----------------|
| 简单函数开销       | 零              | 零              | 零              |
| 复杂函数可读性     | ⭐⭐⭐ 逐行清晰  | ⭐⭐ 缩进块      | ⭐⭐⭐ 分层清晰   |
| Agent 解析难度     | ⭐⭐⭐ 逐行 parse | ⭐⭐ 块 parse    | ⭐⭐ 混合 parse   |
| 一眼看出纯/不纯    | 需看 effects 行 | 需看 effects 行 | ⭐⭐⭐ 看函数名   |
| 一眼看出错误类型    | 需看 errors 行  | 需看 errors 行  | ⭐⭐⭐ 紧跟返回类型 |
| diff 友好度        | ⭐⭐⭐ 每行独立   | ⭐⭐ 块内修改    | ⭐⭐ 混合         |
| 新维度扩展性       | ⭐⭐⭐ 加一行     | ⭐⭐⭐ 加一行     | ⭐⭐ 取决于位置    |
| 与现有语言的相似度  | 低（新设计）     | 高（Haskell）   | 中（Roc-like）   |
