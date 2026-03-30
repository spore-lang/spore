# 函数签名语法 — 混合式方案（确定版 v0.2）

## 设计原则
1. 错误是特殊返回类型 → 紧跟 `->` 用 `!` 分隔
2. 泛型约束 → `where`（Rust 风格）；效果 → `with`；资源 → `uses`；代价 → `cost`，各自独立子句
3. 简单纯函数零开销 → 无 where、无 `!`
4. 编译器推断并显示所有省略的元数据

---

## 语法模板

```
fn <name>[<generics>](<params>) -> <ReturnType> [! [<ErrorTypes>]]
[where <GenericName>: <Constraint>, ...]
[with [<effect1>, <effect2>, ...]]
[uses [<Capability>, ...]]
[cost ≤ <N>]
{
    <body>
}
```

---

## 示例：从简到繁

### 1. 自由函数（最简形式）

```
fn add(a: Int, b: Int) -> Int {
    a + b
}
```

编译器推断输出：
```
  with [pure, deterministic, total]
  cost = 1
  uses []
```

---

### 2. 有错误的纯函数

```
fn parse_int(input: String) -> Int ! [InvalidFormat] {
    ...
}
```

编译器推断输出：
```
  with [pure, deterministic]
  cost = 12
  uses [Compute]
```

---

### 3. 带泛型约束

```
fn serialize<T>(value: T) -> Bytes ! [SerializeError]
where T: Serialize
with [pure, deterministic]
uses [Compute]
cost ≤ 500
{
    ...
}
```

---

### 4. 中等复杂度

```
fn parse_config(raw: String, strict: Bool) -> Config ! [MalformedInput, MissingField]
with [pure, deterministic]
uses [Compute, Module<toml>]
cost ≤ 200
{
    ...
}
```

---

### 5. 有副作用的函数

```
fn sync_user_data(user_id: UserId, source: DataSource) -> SyncReport ! [NetworkTimeout, AuthExpired, DataConflict]
with [idempotent]
uses [NetRead, NetWrite, StateRead, Module<auth>, FuncCall<merge_records>]
cost ≤ 8500
{
    ...
}
```

---

### 6. 带自定义能力

```
capability DatabaseAccess = [NetRead, NetWrite, StateRead, StateWrite]
capability Analytics = [Compute, FuncCall<aggregate>, Module<stats>]

fn generate_report(org_id: OrgId, period: DateRange) -> Report ! [ConnectionLost, QueryTimeout, InsufficientData]
with [deterministic]
uses [DatabaseAccess, Analytics, Module<formatter>]
cost ≤ 12000
{
    ...
}
```

---

### 7. 泛型 + 多约束

```
fn merge<T, U, V>(left: List<T>, right: List<U>, resolver: Fn(T, U) -> V) -> List<V> ! [ConflictError]
where T: Eq + Hash
where U: Eq + Hash
where V: Serialize
with [pure, deterministic]
uses [Compute, FuncCall<resolver>]
cost ≤ 800
{
    ...
}
```

---

### 8. Hole（部分定义，可模拟执行）

```
fn validate_payment(amount: Money, method: PaymentMethod) -> Receipt ! [Declined, InsufficientFunds]
with [idempotent]
uses [NetRead, Module<payment_gateway>]
{
    ?validate_logic
}
```

模拟执行输出：
```json
{
  "hole": "validate_logic",
  "expected_type": "Receipt",
  "bindings": {
    "amount": "Money",
    "method": "PaymentMethod"
  },
  "available_capabilities": ["NetRead", "Module<payment_gateway>"],
  "candidate_functions": [
    "payment_gateway.charge(amount: Money, method: PaymentMethod) -> Receipt ! [Declined, InsufficientFunds]"
  ],
  "error_types_to_handle": ["Declined", "InsufficientFunds"]
}
```

---

### 9. 无错误但有副作用

```
fn log_event(event: Event) -> Unit
with [idempotent]
uses [FileWrite, Clock]
{
    ...
}
```

注意：无 `!` 表示此函数不会失败。

---

### 10. 不完整函数（未声明 uses，有能力依赖）

```
fn fetch_data(url: Url) -> Data ! [NetworkError] {
    http.get(url)    -- 调用了 http 模块
}
```

编译器输出：
```
ERROR [incomplete-function] fetch_data 是不完整函数：
  检测到能力依赖但未声明 `uses`。
  推断能力集: [NetRead, Module<http>]
  
  建议添加:
    uses [NetRead, Module<http>]
  
  当前状态: 可模拟执行，不可真实执行
```

---

## 签名子句的排列顺序（约定）

```
-- 1. 泛型约束（Rust 风格，每条独立一行）
where T: Serialize + Eq
where U: Display

-- 2. 效果标注
with [pure, deterministic]

-- 3. 资源/能力集
uses [Compute, Module<parser>]

-- 4. 代价上界
cost ≤ 500
```

顺序不强制，但编译器格式化输出会遵循此约定。

---

## 效果标注可选值

| 标注 | 含义 | 编译器验证 |
|------|------|-----------|
| `pure` | 无任何副作用 | uses 中不能有 IO/State 类能力 |
| `deterministic` | 相同输入总返回相同输出 | uses 中不能有 Random/Clock |
| `idempotent` | 多次调用效果等同于一次 | 静态检查 + 运行时契约 |
| `total` | 对所有输入都终止 | 编译器验证终止性 |

效果之间的蕴含关系：`pure` ⊃ `deterministic`（pure 必然 deterministic）

---

## Snapshot Hash 覆盖范围（最终版）

以下任一变更 → 新 hash → 需要 `--permit`：

| 签名组件 | 示例变更 |
|----------|---------|
| 函数名 | `parse_config` → `load_config` |
| 参数名 | `raw` → `input` |
| 参数顺序 | `(a, b)` → `(b, a)` |
| 参数类型 | `String` → `Bytes` |
| 返回类型 | `Config` → `Settings` |
| 错误类型集合 | 增删任一错误类型 |
| 效果标注 | `pure` → `idempotent` |
| 代价上界 | `≤ 200` → `≤ 300` |
| 能力集 | 增删任一能力 |
| 泛型约束 | `T: Eq` → `T: Eq + Hash` |
