# Spore 语言语法规范 v0.1

**版本**: 0.1
**日期**: 2024
**状态**: Draft

---

## 1. 概述 (Overview)

Spore 是一门**表达式为中心**（expression-based）的编程语言，设计理念深受 Rust、OCaml 和 Roc 影响，强调**函数式编程范式**、**静态类型系统**、**能力安全**（capability-based security）和**资源成本追踪**。

### 1.1 核心设计原则

1. **一切皆表达式** (Expression-based)
   控制流结构（`if`、`match`）都返回值，代码块的最后一个表达式即为块的值。

2. **花括号块作用域** (Braces for blocks)
   使用 `{}` 明确定义代码块和作用域。

3. **分号语义** (Semicolon semantics)
   采用 Rust 语义：
   - 有分号 `;` → 语句（statement），丢弃值
   - 无分号 → 表达式（expression），返回值

4. **管道操作符** (Pipe operator)
   使用 `|>` 进行数据流链式调用，提高可读性。

5. **固定操作符集** (No custom operators)
   不允许自定义操作符，确保代码可预测性。

6. **字符串插值** (String interpolation)
   - `f"Hello {name}"` — 格式化字符串（f-string）
   - `t"Hello {name}"` — 模板对象（t-string）

7. **错误契约** (Checked error contracts)
   - 函数签名中使用 `! [ErrorType]` 声明闭合错误集合
   - `throw expr` 只能抛出当前函数 `! [...]` 中已声明的错误
   - 调用 `! [E]` 函数时，调用者必须声明兼容错误集合；`?` 只是该传播规则的语法糖

8. **Lambda 表达式** (Lambda)
   Rust 风格闭包语法：`|x, y| x + y`

9. **注释** (Comments)
   - `//` 行注释
   - `///` 文档注释
   - `/* ... */` 块注释（可嵌套）

10. **不可变绑定 + 遮蔽** (Immutable bindings + shadowing)
    使用 `let` 声明不可变绑定，支持遮蔽（shadowing）。可变性通过 `Ref[T]` 容器实现（需要 `StateWrite` capability）。

11. **模式匹配** (Pattern matching)
    使用 `match` 关键字，必须穷尽（exhaustive），支持嵌套、守卫、或模式。

12. **无循环** (No loops)
    通过递归 + 高阶函数（`map`/`fold`/`filter`）替代 `for`/`while` 循环。

13. **尾调用优化** (Tail-call optimization)
    编译器保证尾调用优化（TCO），递归无栈溢出风险。

14. **后缀类型注解** (Postfix type annotations)
    使用 `name: Type` 语法。

15. **自动派生** (Auto-deriving)
    在类型声明处使用 `deriving [...]` 自动派生 trait 实现。手动实现使用 `impl Trait for Type { ... }` 块。

---

## 2. 词法结构 (Lexical Structure)

### 2.1 关键字 (Keywords)

Spore 语言的保留关键字列表：

```
fn          let         if          else        match
struct      type        capability  pub         import
alias       spawn       select      parallel_scope
where       with        uses        effects     cost
return
trait       impl        as          in          mut
const       static      async       await       move
ref         self        super       crate       enum
union       unsafe      extern      macro       mod
true        false       Some        None        Ok
Err         Result      Option      Ref         deriving
```

### 2.2 操作符 (Operators)

| 类别 | 操作符 | 说明 |
|------|--------|------|
| **算术** (Arithmetic) | `+` `-` `*` `/` `%` | 加、减、乘、除、取模 |
| **比较** (Comparison) | `==` `!=` `<` `>` `<=` `>=` | 相等、不等、小于、大于、小于等于、大于等于 |
| **逻辑** (Logical) | `&&` `||` `!` | 与、或、非 |
| **位运算** (Bitwise) | `&` `|` `^` `~` `<<` `>>` | 按位与、或、异或、取反、左移、右移 |
| **管道** (Pipe) | `|>` | 数据流管道 |
| **错误传播** (Error) | `?` | 按已声明错误集合传播错误 |
| **范围** (Range) | `..` `..=` | 半开区间、闭区间 |
| **字段访问** (Field) | `.` | 结构体字段/方法访问 |
| **赋值** (Assignment) | `=` | 绑定赋值 |
| **函数调用** (Call) | `()` | 函数/方法调用 |
| **索引** (Index) | `[]` | 索引访问 |

### 2.3 字面量 (Literals)

#### 2.3.1 整数字面量 (Integer literals)

```spore
// 十进制 (Decimal)
42
-123
1_000_000  // 下划线分隔符

// 十六进制 (Hexadecimal)
0xFF
0xDEADBEEF

// 八进制 (Octal)
0o755

// 二进制 (Binary)
0b1010_1100

// 类型后缀 (Type suffixes)
42i32
100u64
0xFFu8
```

#### 2.3.2 浮点数字面量 (Float literals)

```spore
3.14
-0.001
1.0e-10
2.5e+3
1_000.5
3.14f32
2.718f64
```

#### 2.3.3 布尔字面量 (Boolean literals)

```spore
true
false
```

#### 2.3.4 字符串字面量 (String literals)

```spore
// 普通字符串 (Normal string)
"Hello, World!"
"多行字符串\n可以包含换行"

// 原始字符串 (Raw string) - 不转义
r"C:\Users\path\to\file"
r"正则: \d+\.\d+"

// f-string - 格式化字符串插值 (Formatted string interpolation)
let name = "Alice";
let age = 30;
f"My name is {name} and I am {age} years old"
f"Calculation: {2 + 2}"

// t-string - 模板对象 (Template object)
let template = t"Hello {name}, welcome to {place}";
// 返回一个模板对象，可稍后绑定参数

// 多行字符串 (Multi-line string)
"This is a
multi-line
string"
```

#### 2.3.5 字符字面量 (Character literals)

```spore
'a'
'世'
'\n'
'\t'
'\u{1F600}'  // Unicode escape
```

### 2.4 注释 (Comments)

```spore
// 这是单行注释 (Line comment)

/// 这是文档注释 (Doc comment)
/// 用于生成 API 文档
/// 支持 Markdown 语法
///
/// # 示例 (Example)
/// ```spore
/// let x = add(1, 2);
/// ```
fn add(a: I32, b: I32) -> I32 {
    a + b
}

/* 这是块注释 (Block comment)
   可以跨越多行
   /* 支持嵌套 (Nested) */
   块注释内容
*/
```

### 2.5 标识符 (Identifiers)

Spore 遵循以下命名约定：

- **snake_case**: 变量、函数、模块名
  ```spore
  let user_name = "Alice";
  fn calculate_total() { ... }
  import http.client as http_client
  ```

- **PascalCase**: 类型、capability、枚举变体
  ```spore
  struct UserAccount { ... }
  type Option[T] = Some(T) | None;
  capability Readable { ... }
  ```

- **SCREAMING_SNAKE_CASE**: 常量
  ```spore
  const MAX_BUFFER_SIZE: I32 = 1024;
  const PI: F64 = 3.14159265359;
  ```

---

## 3. 类型定义 (Type Definitions)

### 3.1 结构体 (Struct)

#### 3.1.1 命名字段结构体 (Named-field struct)

```spore
/// 二维平面上的点 (Point on 2D plane)
struct Point {
    x: F64,
    y: F64,
}

/// 用户账户 (User account)
struct User {
    id: U64,
    name: Str,
    email: Str,
    age: U32,
} deriving [Debug, Serialize]
```

#### 3.1.2 元组结构体 (Tuple struct)

```spore
/// RGB 颜色 (RGB color)
struct Color(U8, U8, U8);

/// 包装类型 (Wrapper type)
struct UserId(U64);
```

#### 3.1.3 单元结构体 (Unit struct)

```spore
/// 标记类型 (Marker type)
struct NoData;
```

### 3.2 类型别名与代数数据类型 (Type alias & ADT)

#### 3.2.1 简单类型别名 (Simple type alias)

```spore
type UserId = U64;
type Callback = (I32) -> Str;
```

#### 3.2.2 枚举与求和类型 (Enum & sum type)

```spore
/// 选项类型 (Option type)
type Option[T] =
    | Some(T)
    | None;

/// 结果类型 (Result type)
type Result[T, E] =
    | Ok(T)
    | Err(E);

/// 形状类型 (Shape type)
type Shape =
    | Circle(center: Point, radius: F64)
    | Rectangle(top_left: Point, width: F64, height: F64)
    | Triangle(p1: Point, p2: Point, p3: Point);

/// 二叉树 (Binary tree)
type BinaryTree[T] =
    | Leaf(T)
    | Node(left: BinaryTree[T], value: T, right: BinaryTree[T])
    | Empty;
```

### 3.3 Capability 定义 (Capability definition)

Capability 是 Spore 的 trait/interface 机制，同时也是能力系统的一部分。

```spore
/// 可显示 capability (Display capability)
capability Display {
    fn to_string(self) -> Str;
}

/// 可序列化 capability (Serializable capability)
capability Serialize {
    fn serialize(self) -> Str ! [SerializeError];
}

/// 带关联类型的 capability (Capability with associated type)
capability Collection {
    type Item;

    fn len(self) -> U64;
    fn is_empty(self) -> Bool {
        self.len() == 0  // 默认实现 (Default implementation)
    }
    fn get(self, index: U64) -> Option[Self.Item];
}

/// 可比较 capability (Comparable capability)
capability Comparable {
    fn compare(self, other: Self) -> Ordering;

    fn less_than(self, other: Self) -> Bool {
        match self.compare(other) {
            Ordering.Less => true,
            _ => false,
        }
    }
}
```

### 3.4 泛型类型 (Generic types)

```spore
/// 泛型结构体 (Generic struct)
struct Pair[A, B] {
    first: A,
    second: B,
}

/// 带约束的泛型 (Generic with constraints)
struct SortedList[T] where T: Comparable {
    items: List[T],
}

/// 多约束泛型 (Multiple constraints)
struct Cache[K, V]
where
    K: Hashable + Comparable,
    V: Clone
{
    data: Map[K, V],
    capacity: U64,
}
```

### 3.5 Const 泛型 (Const generics)

```spore
/// 固定大小数组 (Fixed-size array)
struct Array[T, const N: U64] {
    data: List[T],  // 实际长度保证为 N
}

/// 固定大小矩阵 (Fixed-size matrix)
struct Matrix[T, const ROWS: U64, const COLS: U64] {
    data: Array[Array[T, COLS], ROWS],
}

// 使用示例 (Usage example)
let vec3: Array[F64, 3] = Array.new([1.0, 2.0, 3.0]);
let identity: Matrix[F64, 3, 3] = Matrix.identity();
```

### 3.6 Refinement 类型语法 (Refinement type syntax)

```spore
/// 非空字符串 (Non-empty string)
type NonEmptyStr = Str if |s| s.len() > 0;

/// 正整数 (Positive integer)
type PositiveInt = I32 if |n| n > 0;

/// 范围约束 (Range constraint)
type Percentage = F64 if |p| p >= 0.0 && p <= 100.0;

/// 偶数 (Even number)
type EvenInt = I32 if |n| n % 2 == 0;
```

---

## 4. 表达式 (Expressions)

### 4.1 字面量表达式 (Literal expressions)

```spore
42
3.14
true
"Hello"
f"Result: {x}"
```

### 4.2 变量与字段访问 (Variables & field access)

```spore
let x = 10;
x

let point = Point { x: 3.0, y: 4.0 };
point.x
point.y

let user = User { id: 1, name: "Alice", email: "alice@example.com", age: 30 };
user.name
```

### 4.3 块表达式 (Block expressions)

```spore
// 块的最后一个表达式是块的值 (Last expression is block value)
let result = {
    let a = 10;
    let b = 20;
    a + b  // 无分号，返回此值
};
// result == 30

// 带语句的块 (Block with statements)
let side_effect = {
    let x = compute();  // 语句
    print(f"Computed: {x}");  // 语句
    x * 2  // 返回值
};
```

### 4.4 条件表达式 (Conditional expressions)

```spore
/// 简单 if-else (Simple if-else)
let abs_value = if x >= 0 { x } else { -x };

/// 多分支 if-else (Multi-branch if-else)
let sign = if x > 0 {
    "positive"
} else if x < 0 {
    "negative"
} else {
    "zero"
};

/// if 作为语句（有分号，丢弃值）(if as statement)
if user.is_admin {
    log("Admin access granted");
};  // 分号表示丢弃返回值

/// 嵌套 if (Nested if)
let category = if age < 13 {
    "child"
} else if age < 20 {
    "teenager"
} else if age < 60 {
    "adult"
} else {
    "senior"
};
```

### 4.5 模式匹配表达式 (Pattern matching expressions)

```spore
/// 基本 match (Basic match)
let result = match option {
    Some(value) => value,
    None => 0,
};

/// 多分支 match (Multi-branch match)
let message = match shape {
    Circle(center, radius) => f"Circle at {center} with radius {radius}",
    Rectangle(tl, w, h) => f"Rectangle: {w}x{h}",
    Triangle(p1, p2, p3) => "Triangle",
};

/// 守卫子句 (Guard clause)
let category = match age {
    n if n < 0 => "invalid",
    n if n < 13 => "child",
    n if n < 20 => "teenager",
    n if n < 60 => "adult",
    _ => "senior",
};

/// 或模式 (Or-pattern)
let is_weekend = match day {
    "Saturday" | "Sunday" => true,
    _ => false,
};

/// 嵌套模式 (Nested pattern)
let result = match result {
    Ok(Some(value)) => f"Found: {value}",
    Ok(None) => "Not found",
    Err(e) => f"Error: {e}",
};

/// 结构体模式 (Struct pattern)
let distance = match point {
    Point { x: 0.0, y: 0.0 } => 0.0,
    Point { x, y } => sqrt(x * x + y * y),
};

/// 列表模式 (List pattern)
let description = match list {
    [] => "empty",
    [single] => f"one element: {single}",
    [first, second] => f"two elements: {first}, {second}",
    [head, ..tail] => f"head: {head}, tail length: {tail.len()}",
};
```

### 4.6 函数调用 (Function calls)

```spore
// 普通函数调用 (Regular function call)
let sum = add(10, 20);

// 方法调用 (Method call)
let length = text.len();
let upper = text.to_uppercase();

// 链式调用 (Method chaining)
let result = text
    .trim()
    .to_lowercase()
    .split(" ");
```

### 4.7 管道操作符 (Pipe operator)

```spore
/// 基本管道 (Basic pipe)
let result = data
    |> parse
    |> validate
    |> process
    |> format;

/// 等价于 (Equivalent to)
let result = format(process(validate(parse(data))));

/// 带参数的管道 (Pipe with arguments)
let result = numbers
    |> filter(|x| x > 0)
    |> map(|x| x * 2)
    |> fold(0, |acc, x| acc + x);

/// 复杂管道示例 (Complex pipe example)
let output = input
    |> trim
    |> to_lowercase
    |> split(" ")
    |> filter(|word| word.len() > 3)
    |> map(|word| capitalize(word))
    |> join(", ");
```

### 4.8 Lambda 表达式 (Lambda expressions)

```spore
/// 单参数 lambda (Single parameter)
let double = |x| x * 2;

/// 多参数 lambda (Multiple parameters)
let add = |a, b| a + b;

/// 带类型注解的 lambda (Lambda with type annotations)
let multiply: (I32, I32) -> I32 = |a: I32, b: I32| a * b;

/// 带块体的 lambda (Lambda with block body)
let complex_fn = |x, y| {
    let sum = x + y;
    let product = x * y;
    sum * product
};

/// 闭包捕获变量 (Closure capturing variables)
let threshold = 10;
let filter_fn = |x| x > threshold;  // 捕获 threshold

/// 高阶函数中的 lambda (Lambda in higher-order functions)
let doubled = numbers.map(|x| x * 2);
let evens = numbers.filter(|x| x % 2 == 0);
let sum = numbers.fold(0, |acc, x| acc + x);
```

### 4.9 错误传播操作符 (Error propagation operator)

```spore
/// 基本用法 (Basic usage)
fn read_config(path: Str) -> Config ! [IoError, ParseError] {
    let content = read_file(path)?;  // 如果失败，立即返回错误
    let config = parse_config(content)?;  // 同上
    config  // 成功则返回配置
}

/// 链式传播 (Chained propagation)
fn process_data(input: Str) -> Result ! [ValidationError, ProcessError] {
    let validated = validate(input)?;
    let transformed = transform(validated)?;
    let result = finalize(transformed)?;
    Ok(result)
}

/// 与管道结合 (Combined with pipe)
fn pipeline(data: Data) -> Output ! [Error] {
    data
        |> step1?
        |> step2?
        |> step3?
}
```

`?` 不会绕过错误检查。若被调用函数的签名包含 `! [E]`，则当前函数要么本地处理该错误，要么在自己的 `! [...]` 中声明兼容集合后再使用 `?` 传播。

### 4.10 Ref 操作 (Ref operations)

`Ref[T]` 是 Spore 的可变容器类型，需要 `StateWrite` capability。

```spore
/// 创建可变引用 (Create mutable reference)
let counter = Ref.new(0);

/// 读取值 (Read value)
let current = counter.get();

/// 设置值 (Set value)
counter.set(current + 1);

/// 修改值 (Update value)
counter.update(|x| x + 1);

/// 在函数中使用 Ref (Using Ref in functions)
fn increment_counter(counter: Ref[I32]) {
    let current = counter.get();
    counter.set(current + 1);
}
```

---

## 5. 函数定义 (Function Definitions)

### 5.1 完整函数签名 (Full function signature)

```spore
/// 完整签名语法 (Full signature syntax)
fn function_name[TypeParam1, TypeParam2](
    param1: Type1,
    param2: Type2,
) -> ReturnType ! [Error1, Error2]
where TypeParam1: Constraint1
where TypeParam2: Constraint2
uses [resource1, resource2]
cost [compute, alloc, io, parallel]
spec {
    example "baseline" => function_name(sample1, sample2) == expected
}
{
    // 函数体 (Function body)
    body_expression
}
```

说明：稳定语法只支持单一约束 `where T: Trait`。若同一函数需要多个约束，请重复书写多行 `where`；逗号分组、`+` 组合和 `where { ... }` 形式都不在 v0.1 范围内。`cost` 子句固定写成 `cost [compute, alloc, io, parallel]`，四个位置依次表示计算、分配、IO 与并行宽度。当前每个槽位只接受最小表达式子集：整数常量、参数变量或线性 `O(n)`；`log`/`max`/`min`、成员访问（如 `urls.len`）以及更丰富的代数组合都留待后续版本。

### 5.2 简单函数 (Simple functions)

```spore
/// 最简函数 (Minimal function)
fn add(a: I32, b: I32) -> I32 {
    a + b
}

/// 无参数函数 (No parameters)
fn get_pi() -> F64 {
    3.14159265359
}

/// 无返回值函数（显式返回 `()`）(No return value - returns unit)
fn print_hello() -> () {
    print("Hello, World!");
}

/// 单表达式函数 (Single-expression function)
fn double(x: I32) -> I32 {
    x * 2
}
```

### 5.3 泛型函数 (Generic functions)

```spore
/// 泛型身份函数 (Generic identity function)
fn identity[T](x: T) -> T {
    x
}

/// 带约束的泛型函数 (Generic with constraints)
fn max[T](a: T, b: T) -> T
where T: Comparable
{
    if a.compare(b) == Ordering.Greater { a } else { b }
}

/// 多类型参数 (Multiple type parameters)
fn pair[A, B](first: A, second: B) -> Pair[A, B] {
    Pair { first, second }
}

/// 泛型集合操作 (Generic collection operation)
fn map[A, B](list: List[A], f: (A) -> B) -> List[B] {
    match list {
        [] => [],
        [head, ..tail] => [f(head), ..map(tail, f)],
    }
}
```

### 5.4 带效应声明的函数 (Functions with effects)

```spore
/// 声明 I/O 效应 (Declaring I/O effects)
fn read_file(path: Str) -> Str ! [IoError]
uses [FileRead]
{
    // 实现代码 (Implementation)
    ?implementation
}

/// 声明网络效应 (Declaring network effects)
fn fetch_data(url: Str) -> Data ! [NetworkError]
uses [Network]
{
    ?implementation
}

/// 声明状态效应 (Declaring state effects)
fn increment(ref: Ref[I32])
uses [StateRead, StateWrite]
{
    let current = ref.get();
    ref.set(current + 1);
}
```

### 5.5 带成本约束的函数 (Functions with cost constraints)

当前文档中的 `cost` 示例只展示三类允许的槽位写法：整数常量、参数变量、线性 `O(n)`。

```spore
/// O(1) 常数时间操作 (O(1) constant time)
fn array_get[T](arr: Array[T, N], index: U64) -> Option[T]
cost [10, 1, 0, 0]
{
    if index < N { Some(arr.data[index]) } else { None }
}

/// O(n) 线性时间操作 (O(n) linear time)
fn sum_list(list: List[I32], n: I32) -> I32
cost [O(n), 0, 0, 0]
{
    list.fold(0, |acc, x| acc + x)
}

/// 递归函数的成本 (Recursive function cost)
fn factorial(n: I32) -> I32
cost [O(n), 0, 0, 0]
{
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}
```

### 5.6 带资源依赖的函数 (Functions with resource dependencies)

```spore
/// 声明资源依赖 (Declaring resource dependencies)
fn query_database(sql: Str) -> Result[Data] ! [DbError]
uses [Database, db_connection]
{
    ?implementation
}

/// 多资源依赖 (Multiple resource dependencies)
fn process_request(req: Request) -> Response ! [Error]
uses [Network, Database, FileRead, db_pool, cache, logger]
{
    ?implementation
}
```

### 5.7 文档注释函数 (Functions with doc comments)

```spore
/// 计算两个数的最大公约数 (Calculate GCD of two numbers)
///
/// # 参数 (Parameters)
/// - `a`: 第一个整数 (First integer)
/// - `b`: 第二个整数 (Second integer)
///
/// # 返回值 (Returns)
/// 返回 a 和 b 的最大公约数 (Returns the GCD of a and b)
///
/// # 示例 (Example)
/// ```spore
/// let result = gcd(48, 18);  // result == 6
/// ```
fn gcd(a: I32, b: I32) -> I32
{
    if b == 0 { a } else { gcd(b, a % b) }
}
```

---

## 6. 模式匹配 (Pattern Matching)

### 6.1 模式类型 (Pattern types)

#### 6.1.1 字面量模式 (Literal patterns)

```spore
match x {
    0 => "zero",
    1 => "one",
    2 => "two",
    _ => "other",
}

match status {
    "ok" => handle_success(),
    "error" => handle_error(),
    _ => handle_unknown(),
}
```

#### 6.1.2 变量绑定模式 (Variable binding patterns)

```spore
match option {
    Some(value) => f"Got value: {value}",
    None => "No value",
}

match result {
    Ok(data) => process(data),
    Err(e) => log_error(e),
}
```

#### 6.1.3 通配符模式 (Wildcard pattern)

```spore
match shape {
    Circle(_, radius) => f"Circle with radius {radius}",
    Rectangle(_, width, height) => f"Rectangle {width}x{height}",
    Triangle(_, _, _) => "Triangle",
}
```

#### 6.1.4 构造器模式 (Constructor patterns)

```spore
/// 匹配枚举变体 (Matching enum variants)
match tree {
    Leaf(value) => value,
    Node(left, value, right) => value,
    Empty => 0,
}

/// 命名字段构造器 (Named-field constructor)
match shape {
    Circle(center: Point { x, y }, radius) =>
        f"Circle at ({x}, {y}) radius {radius}",
    _ => "Other shape",
}
```

#### 6.1.5 结构体模式 (Struct patterns)

```spore
/// 完整字段匹配 (Full field matching)
match point {
    Point { x: 0.0, y: 0.0 } => "origin",
    Point { x: 0.0, y } => f"on y-axis at {y}",
    Point { x, y: 0.0 } => f"on x-axis at {x}",
    Point { x, y } => f"at ({x}, {y})",
}

/// 字段省略（field punning）(Field punning)
match user {
    User { name, age, .. } => f"{name} is {age} years old",
}
```

#### 6.1.6 列表模式 (List patterns)

```spore
/// 空列表与非空列表 (Empty vs non-empty)
match list {
    [] => "empty",
    [_] => "single element",
    [_, _] => "two elements",
    _ => "many elements",
}

/// 头尾解构 (Head-tail destructuring)
match list {
    [] => 0,
    [head, ..tail] => head + sum(tail),
}

/// 多元素解构 (Multi-element destructuring)
match list {
    [first, second, ..rest] => first + second,
    [single] => single,
    [] => 0,
}
```

### 6.2 守卫子句 (Guard clauses)

```spore
/// 带条件的模式 (Pattern with condition)
match number {
    n if n < 0 => "negative",
    n if n == 0 => "zero",
    n if n > 0 && n < 10 => "small positive",
    n if n >= 10 => "large positive",
    _ => unreachable,
}

/// 复杂守卫 (Complex guard)
match user {
    User { age, is_verified: true, .. } if age >= 18 =>
        "verified adult",
    User { age, .. } if age >= 18 =>
        "unverified adult",
    _ =>
        "minor",
}
```

### 6.3 或模式 (Or-patterns)

```spore
/// 多值匹配 (Multiple value matching)
match day {
    "Mon" | "Tue" | "Wed" | "Thu" | "Fri" => "weekday",
    "Sat" | "Sun" => "weekend",
    _ => "invalid",
}

/// 枚举变体或模式 (Enum variant or-pattern)
match status {
    Status.Success | Status.PartialSuccess => "ok",
    Status.Failure | Status.Error => "failed",
}
```

### 6.4 嵌套模式 (Nested patterns)

```spore
/// 深层嵌套 (Deep nesting)
match result {
    Ok(Some(User { name, age })) =>
        f"User {name}, age {age}",
    Ok(Some(_)) =>
        "User with incomplete data",
    Ok(None) =>
        "No user found",
    Err(e) =>
        f"Error: {e}",
}

/// 列表嵌套 (List nesting)
match pairs {
    [[a, b], [c, d]] => a + b + c + d,
    _ => 0,
}
```

### 6.5 穷尽性检查 (Exhaustiveness checking)

```spore
/// 编译器强制穷尽匹配 (Compiler enforces exhaustive matching)
type Color = Red | Green | Blue;

// ✅ 正确：穷尽所有情况 (Correct: all cases covered)
match color {
    Red => "red",
    Green => "green",
    Blue => "blue",
}

// ❌ 错误：缺少 Blue 分支 (Error: missing Blue branch)
// match color {
//     Red => "red",
//     Green => "green",
// }

/// 使用通配符确保穷尽 (Using wildcard to ensure exhaustiveness)
match color {
    Red => "red",
    _ => "not red",  // 覆盖 Green 和 Blue
}
```

---

## 7. 模块与导入 (Modules & Imports)

### 7.1 文件即模块 (File = module)

每个 `.spore` 源文件就是一个模块，模块名由其相对 `src/` 的文件路径决定：

| 文件路径 | 模块名 |
|----------|--------|
| `src/math.spore` | `math` |
| `src/geometry/shapes.spore` | `geometry.shapes` |
| `src/http/client.spore` | `http.client` |

Spore **没有** `module ...` 文件头，也没有内联/嵌套模块声明语法。

### 7.2 可见性修饰符 (Visibility modifiers)

```spore
/// 公开（所有模块可见）(Public - visible to all modules)
pub fn public_function() { }

/// 包内可见（同一 package 内可见）(Package-visible)
pub(pkg) fn package_function() { }

/// 私有（默认，仅当前文件模块可见）(Private - default, only visible in current file module)
fn private_function() { }
```

### 7.3 导入语句 (Import statements)

```spore
/// 导入模块并重命名 (Import module with alias)
import std.collections as collections
import very.long.module.name as short

/// 别名特定项 (Alias specific item)
alias Vec = std.collections.Vector
alias HashMap = std.collections.HashMap
```

说明：

- `import` 仅用于模块路径，可选 `as` 别名。
- `alias` 仅用于具体项绑定。
- 当前文档**不支持** `import foo.{bar}`、`import foo.*` 或嵌套 `module` 语法。

### 7.4 无模块级 capability carrier (No module-level capability carrier)

模块名仅由文件路径决定，源码中没有 `module ...` 文件头，也没有模块级 `uses` / capability ceiling 语法。
能力检查只发生在函数签名的 `uses [...]` 与项目 / Platform 边界；模块导入本身不会携带或放宽能力。

---

## 8. 并发语法 (Concurrency Syntax)

### 8.1 并发作用域 (Parallel scope)

```spore
/// 基本并发作用域 (Basic parallel scope)
parallel_scope {
    spawn { task1() };
    spawn { task2() };
    spawn { task3() };
}  // 等待所有 spawn 任务完成 (Waits for all spawned tasks)

/// 带返回值的并发 (Parallel with return values)
let results = parallel_scope {
    let a = spawn { compute_a() };
    let b = spawn { compute_b() };
    let c = spawn { compute_c() };

    [a.await, b.await, c.await]  // 收集结果 (Collect results)
};
```

### 8.2 Channel 通信 (Channel communication)

```spore
/// 创建 channel (Create channel)
let (tx, rx) = Channel.new[I32](buffer: 10);

/// 发送数据 (Send data)
parallel_scope {
    spawn {
        tx.send(42);
        tx.send(100);
    };

    spawn {
        let value1 = rx.recv();  // 接收数据 (Receive data)
        let value2 = rx.recv();
        print(f"Received: {value1}, {value2}");
    };
}

/// 多生产者单消费者 (Multiple producers single consumer)
let (tx, rx) = Channel.new[Str](buffer: 5);

parallel_scope {
    let tx1 = tx.clone();
    let tx2 = tx.clone();

    spawn {
        tx1.send("from producer 1");
    };

    spawn {
        tx2.send("from producer 2");
    };

    spawn {
        let msg1 = rx.recv();
        let msg2 = rx.recv();
        print(f"{msg1}, {msg2}");
    };
}
```

### 8.3 Select 表达式 (Select expression)

```spore
/// 多路复用 channel (Multiplex channels)
let (tx1, rx1) = Channel.new[I32](buffer: 1);
let (tx2, rx2) = Channel.new[Str](buffer: 1);

parallel_scope {
    /// 递归事件循环 (Recursive event loop with TCO)
    fn event_loop(rx1: Receiver[I32], rx2: Receiver[Str]) {
        select {
            value from rx1 => {
                print(f"Got integer: {value}");
            },
            message from rx2 => {
                print(f"Got string: {message}");
            },
        }
        event_loop(rx1, rx2)  // 尾递归 (Tail recursion - TCO guaranteed)
    }

    spawn {
        event_loop(rx1, rx2);
    };

    spawn {
        tx1.send(42);
    };

    spawn {
        tx2.send("Hello");
    };
}

/// 带超时的 select (Select with timeout)
select {
    value from rx => {
        print(f"Received: {value}");
    },
    timeout(1.seconds) => {
        print("Timed out after 1 second");
    },
}
```

### 8.4 Task.await 操作 (Task.await operation)

活动规范里的并发表面语法使用后缀形式 `task.await`。旧的前缀 `await expr`
兼容策略仍待单独决策，不影响这里的目标语法。

```spore
/// 等待异步任务完成 (Wait for async task completion)
parallel_scope {
    let task = spawn {
        expensive_computation()
    };

    // 做其他工作 (Do other work)
    do_something_else();

    // 等待结果 (Wait for result)
    let result = task.await;
    print(f"Result: {result}");
}

/// 同时等待多个任务 (Await multiple tasks)
parallel_scope {
    let task1 = spawn { fetch("https://api1.com") };
    let task2 = spawn { fetch("https://api2.com") };
    let task3 = spawn { fetch("https://api3.com") };

    let results = [task1.await, task2.await, task3.await];
    results
}
```

---

## 9. 错误处理 (Error Handling)

### 9.1 错误类型定义 (Error type definition)

```spore
/// 自定义错误类型 (Custom error type)
type FileError =
    | NotFound(path: Str)
    | PermissionDenied(path: Str)
    | IoError(message: Str);

type NetworkError =
    | Timeout
    | ConnectionRefused
    | InvalidResponse(code: I32);

type ParseError =
    | SyntaxError(line: U32, column: U32, message: Str)
    | UnexpectedToken(token: Str)
    | UnexpectedEof;
```

### 9.2 声明可能的错误 (Declaring possible errors)

```spore
/// 单一错误类型 (Single error type)
fn read_file(path: Str) -> Str ! [FileError] {
    ?implementation
}

/// 多种错误类型 (Multiple error types)
fn fetch_and_parse(url: Str) -> Data ! [NetworkError, ParseError] {
    let response = fetch(url)?;  // 可能抛出 NetworkError
    let data = parse(response)?;  // 可能抛出 ParseError
    data
}

/// 泛型错误 (Generic error)
fn try_parse[T, E](input: Str, parser: (Str) -> Result[T, E]) -> T ! [E] {
    match parser(input) {
        Ok(value) => value,
        Err(e) => throw e,  // 抛出错误
    }
}
```

`throw expr` 与 `?` 形成同一闭环：`throw expr` 只有在 `expr` 的错误类型已出现在当前函数 `! [...]` 中时才合法；调用 `! [E]` callee 时，要么在本地处理，要么把 `E` 纳入调用者签名。

### 9.3 错误传播操作符 (Error propagation operator)

`?` 是对“调用一个 `! [E]` 函数并把 `E` 继续暴露给当前调用者”的简写。它不会自动扩展签名；当前函数仍必须显式声明兼容的 `! [...]` 错误集合。

```spore
/// 自动传播错误 (Automatic error propagation)
fn process_file(path: Str) -> Data ! [FileError, ParseError] {
    let content = read_file(path)?;  // `read_file` 的 FileError 已包含在当前签名中
    let data = parse(content)?;      // `parse` 的 ParseError 也已显式声明
    validate(data)?;                 // 仍然要求当前签名兼容被调函数的错误集合
    data
}

/// 错误转换 (Error transformation)
fn load_config(path: Str) -> Config ! [ConfigError] {
    let content = read_file(path)?;  // FileError -> ConfigError
    let config = parse_toml(content)?;  // ParseError -> ConfigError
    config
}
```

### 9.4 Match 错误处理 (Match error handling)

```spore
/// 显式匹配 Result (Explicit Result matching)
fn handle_result() {
    match read_file("config.toml") {
        Ok(content) => {
            print(f"File content: {content}");
        },
        Err(FileError.NotFound(path)) => {
            print(f"File not found: {path}");
        },
        Err(FileError.PermissionDenied(path)) => {
            print(f"Permission denied: {path}");
        },
        Err(FileError.IoError(msg)) => {
            print(f"I/O error: {msg}");
        },
    }
}

/// 嵌套错误处理 (Nested error handling)
fn complex_operation() {
    match fetch_and_parse("https://api.example.com") {
        Ok(data) => process(data),
        Err(NetworkError.Timeout) => retry(),
        Err(NetworkError.ConnectionRefused) => use_fallback(),
        Err(NetworkError.InvalidResponse(code)) =>
            log_error(f"HTTP {code}"),
        Err(ParseError.SyntaxError(line, col, msg)) =>
            log_error(f"Syntax error at {line}:{col}: {msg}"),
        Err(_) => handle_unknown_error(),
    }
}
```

### 9.5 错误恢复 (Error recovery)

```spore
/// 提供默认值 (Provide default value)
fn get_config() -> Config {
    match load_config("config.toml") {
        Ok(config) => config,
        Err(_) => Config.default(),
    }
}

/// 重试逻辑 (Retry logic)
fn fetch_with_retry(url: Str, max_retries: I32) -> Data ! [NetworkError] {
    /// 递归重试 (Recursive retry with TCO)
    fn retry(url: Str, attempts: I32, max_retries: I32) -> Data ! [NetworkError] {
        match fetch(url) {
            Ok(data) => data,
            Err(NetworkError.Timeout) if attempts < max_retries => {
                sleep(1000);
                retry(url, attempts + 1, max_retries)  // 尾递归 (Tail recursion)
            },
            Err(e) => throw e,
        }
    }
    retry(url, 0, max_retries)
}
```

---

## 10. Hole 语法 (Hole Syntax)

Hole 是 Spore 的渐进式开发机制，允许在类型检查下保留未实现部分。

### 10.1 基本 Hole (Basic hole)

```spore
/// 未命名 hole (Unnamed hole)
fn compute() -> I32 {
    ?  // 编译器推断类型为 I32
}

/// 命名 hole (Named hole)
fn process(data: Data) -> Result {
    let validated = validate(data);
    ?process_impl  // 命名 hole，便于追踪
}

/// 带类型注解的 hole (Hole with type annotation)
fn complex_function() -> ComplexType {
    ?result : ComplexType  // 显式类型
}
```

### 10.2 Hole 在函数签名中 (Hole in function signature)

```spore
/// 返回类型 hole (Return type hole)
fn mysterious_function(x: I32) -> ? {
    x * 2 + 10
}  // 编译器推断返回类型为 I32

/// 参数类型 hole (Parameter type hole)
fn generic_wrapper(value: ?) -> Str {
    f"Value: {value}"
}
```

### 10.3 Hole 允许列表 (Hole allow-list)

```spore
/// 使用 @allows 注解限制可用函数 (Use @allows to restrict available functions)
@allows[validate, sanitize, format]
fn process_input(raw: Str) -> Result ! [ValidationError] {
    let validated = validate(raw)?;
    let sanitized = sanitize(validated);
    ?final_step  // 此 hole 只能调用 validate/sanitize/format
}

/// 多个 hole 共享允许列表 (Multiple holes share allow-list)
@allows[add, multiply, negate]
fn arithmetic(a: I32, b: I32) -> I32 {
    let x = ?step1 : I32;  // 只能用 add/multiply/negate
    let y = ?step2 : I32;  // 同上
    x + y
}
```

### 10.4 Hole 与类型推断 (Hole with type inference)

```spore
/// 编译器填充 hole (Compiler fills hole)
fn example() {
    let list = [1, 2, 3, 4, 5];
    let result = list
        |> filter(?)  // hole: |x| x > 0 (编译器可推断)
        |> map(?)     // hole: |x| x * 2
        |> fold(0, ?);  // hole: |acc, x| acc + x
}
```

> **Tooling note**: 面向 Agent / IDE 的协议使用稳定 `hole id` 标识**函数体里的 expression hole**。命名 hole 直接复用源码中的名字；匿名 `?` 由编译器分配稳定 id。函数签名中的 `?` 仅用于类型推断，不单独进入 HoleReport / `--query-hole` 填充协议。

---

## 11. 语法糖与便利功能 (Syntactic Sugar)

### 11.1 字符串插值 (String interpolation)

```spore
/// f-string - 直接格式化 (Immediate formatting)
let name = "Alice";
let age = 30;
let message = f"Hello, {name}! You are {age} years old.";

/// 支持表达式 (Supports expressions)
let x = 10;
let y = 20;
let result = f"Sum: {x + y}, Product: {x * y}";

/// t-string - 模板对象 (Template object)
let template = t"Dear {customer_name}, your order {order_id} is ready.";
// 稍后绑定参数 (Bind parameters later)
let message = template.bind([
    ("customer_name", "Bob"),
    ("order_id", "12345"),
]);
```

### 11.2 字段省略 (Field punning)

```spore
/// 结构体构造时省略同名字段 (Omit same-name fields in struct construction)
let x = 10;
let y = 20;

// 完整形式 (Full form)
let point1 = Point { x: x, y: y };

// 省略形式 (Punning form)
let point2 = Point { x, y };  // 等价于上面

/// 模式匹配中的字段省略 (Field punning in pattern matching)
match point {
    Point { x, y } => f"Point at ({x}, {y})",  // 绑定 x 和 y
}
```

### 11.3 管道操作符变换 (Pipe operator transformations)

```spore
/// 基本变换 (Basic transformation)
x |> f         // 等价于 (equivalent to): f(x)
x |> f |> g    // 等价于: g(f(x))

/// 带参数的变换 (Transformation with arguments)
x |> f(y, z)   // 等价于: f(x, y, z)
x |> f(_, y)   // 等价于: f(x, y)
x |> f(y, _)   // 等价于: f(y, x)

/// 方法调用变换 (Method call transformation)
list |> .map(|x| x * 2)          // 等价于: list.map(|x| x * 2)
text |> .trim() |> .to_uppercase()  // 等价于: text.trim().to_uppercase()
```

### 11.4 Range 语法 (Range syntax)

```spore
/// 半开区间 (Half-open range) [start, end)
let range1 = 1..10;  // 1, 2, 3, ..., 9

/// 闭区间 (Closed range) [start, end]
let range2 = 1..=10;  // 1, 2, 3, ..., 10

/// 在列表操作中使用 (Use in list operations)
let sublist = list[2..5];  // 索引 2, 3, 4
let all_from_3 = list[3..];  // 从索引 3 到末尾
let first_5 = list[..5];  // 前 5 个元素

/// 在迭代中使用 (Use in iteration)
let sum = (1..=100).fold(0, |acc, x| acc + x);
```

### 11.5 方法链 (Method chaining)

```spore
/// 流式 API (Fluent API)
let result = text
    .trim()
    .to_lowercase()
    .split(" ")
    .filter(|word| word.len() > 3)
    .map(|word| capitalize(word))
    .collect();

/// 等价管道形式 (Equivalent pipe form)
let result = text
    |> trim
    |> to_lowercase
    |> split(" ")
    |> filter(|word| word.len() > 3)
    |> map(|word| capitalize(word))
    |> collect;
```

---

## 12. 命名约定 (Naming Conventions)

### 12.1 Snake_case

用于：变量、函数、模块

```spore
let user_count = 42;
let is_valid = true;

fn calculate_total(items: List[Item]) -> F64 { ... }
fn parse_json(input: Str) -> Result[Json] { ... }

import http.client as http_client
import data.processing as data_processing
```

### 12.2 PascalCase

用于：类型、capability、枚举变体

```spore
struct UserAccount { ... }
struct HttpRequest { ... }

type Option[T] = Some(T) | None;
type NetworkError = Timeout | ConnectionRefused;

capability Readable { ... }
capability Serializable { ... }

// 枚举变体 (Enum variants)
Color.Red
Status.Success
```

### 12.3 SCREAMING_SNAKE_CASE

用于：常量

```spore
const MAX_CONNECTIONS: I32 = 100;
const DEFAULT_TIMEOUT: I32 = 5000;
const API_BASE_URL: Str = "https://api.example.com";
const PI: F64 = 3.14159265359;
```

### 12.4 类型参数命名 (Type parameter naming)

```spore
/// 单个字母大写 (Single uppercase letter)
fn identity[T](x: T) -> T { x }
fn pair[A, B](first: A, second: B) -> Pair[A, B] { ... }

/// 描述性名称（PascalCase）(Descriptive names)
fn convert[From, To](value: From, converter: (From) -> To) -> To { ... }
fn cache[Key, Value](key: Key) -> Option[Value] { ... }
```

---

## 13. 完整示例 (Complete Examples)

### 示例 1: HTTP 服务器 (HTTP Server)

```spore
/// HTTP 请求类型 (HTTP request type)
struct Request {
    method: Str,
    path: Str,
    headers: Map[Str, Str],
    body: Str,
}

/// HTTP 响应类型 (HTTP response type)
struct Response {
    status: I32,
    headers: Map[Str, Str],
    body: Str,
}

/// 路由处理器类型 (Route handler type)
type Handler = (Request) -> Response ! [HttpError];

/// HTTP 错误 (HTTP error)
type HttpError =
    | BadRequest(message: Str)
    | NotFound(path: Str)
    | InternalError(message: Str);

/// 路由匹配 (Route matching)
fn route(req: Request) -> Response ! [HttpError] {
    match (req.method, req.path) {
        ("GET", "/") =>
            Ok(Response {
                status: 200,
                headers: Map.from([("Content-Type", "text/html")]),
                body: "<h1>Welcome</h1>",
            }),

        ("GET", path) if path.starts_with("/api/") =>
            handle_api(req),

        ("POST", "/submit") =>
            handle_submit(req),

        (_, path) =>
            Err(HttpError.NotFound(path)),
    }
}

/// API 处理器 (API handler)
fn handle_api(req: Request) -> Response ! [HttpError]
uses [Network, Database]
{
    let data = query_database()?;
    let json = serialize(data)?;

    Ok(Response {
        status: 200,
        headers: Map.from([("Content-Type", "application/json")]),
        body: json,
    })
}

/// 启动服务器 (Start server)
fn start_server(port: I32) ! [IoError]
uses [Network]
{
    let listener = TcpListener.bind(f"127.0.0.1:{port}")?;

    /// 递归接受连接 (Recursive accept loop with TCO)
    fn accept_loop(listener: TcpListener) ! [IoError] {
        let connection = listener.accept()?;

        parallel_scope {
            spawn {
                match route(connection.request) {
                    Ok(response) => connection.send(response),
                    Err(e) => connection.send(error_response(e)),
                }
            };
        }

        accept_loop(listener)  // 尾递归 (Tail recursion - TCO guaranteed)
    }

    accept_loop(listener)
}
```

### 示例 2: 表达式解析器与求值器 (Expression Parser & Evaluator)

```spore
/// 表达式 AST (Expression AST)
type Expr =
    | Literal(I32)
    | Variable(name: Str)
    | BinOp(op: Op, left: Expr, right: Expr)
    | UnaryOp(op: UnaryOp, expr: Expr)
    | Let(name: Str, value: Expr, body: Expr)
    | If(condition: Expr, then_branch: Expr, else_branch: Expr);

/// 二元操作符 (Binary operator)
type Op = Add | Sub | Mul | Div | Equal | LessThan;

/// 一元操作符 (Unary operator)
type UnaryOp = Negate | Not;

/// 环境（变量绑定）(Environment - variable bindings)
type Env = Map[Str, I32];

/// 求值错误 (Evaluation error)
type EvalError =
    | UndefinedVariable(name: Str)
    | DivisionByZero
    | TypeError(message: Str);

/// 求值器 (Evaluator)
fn eval(expr: Expr, env: Env) -> I32 ! [EvalError]
{
    match expr {
        Literal(n) => n,

        Variable(name) => match env.get(name) {
            Some(value) => value,
            None => throw EvalError.UndefinedVariable(name),
        },

        BinOp(op, left, right) => {
            let left_val = eval(left, env)?;
            let right_val = eval(right, env)?;
            eval_binop(op, left_val, right_val)?
        },

        UnaryOp(op, e) => {
            let val = eval(e, env)?;
            match op {
                Negate => -val,
                Not => if val == 0 { 1 } else { 0 },
            }
        },

        Let(name, value_expr, body) => {
            let value = eval(value_expr, env)?;
            let new_env = env.insert(name, value);
            eval(body, new_env)?
        },

        If(cond, then_branch, else_branch) => {
            let cond_val = eval(cond, env)?;
            if cond_val != 0 {
                eval(then_branch, env)?
            } else {
                eval(else_branch, env)?
            }
        },
    }
}

/// 求值二元操作 (Evaluate binary operation)
fn eval_binop(op: Op, left: I32, right: I32) -> I32 ! [EvalError] {
    match op {
        Add => left + right,
        Sub => left - right,
        Mul => left * right,
        Div => if right == 0 {
            throw EvalError.DivisionByZero
        } else {
            left / right
        },
        Equal => if left == right { 1 } else { 0 },
        LessThan => if left < right { 1 } else { 0 },
    }
}

/// 示例用法 (Example usage)
fn example() {
    // let x = 10 in let y = 20 in x + y
    let expr = Expr.Let(
        "x",
        Expr.Literal(10),
        Expr.Let(
            "y",
            Expr.Literal(20),
            Expr.BinOp(Op.Add, Expr.Variable("x"), Expr.Variable("y"))
        )
    );

    let env = Map.empty();

    match eval(expr, env) {
        Ok(result) => print(f"Result: {result}"),  // 输出: Result: 30
        Err(e) => print(f"Error: {e}"),
    }
}
```

### 示例 3: 并发生产者-消费者 (Concurrent Producer-Consumer)

```spore
/// 任务类型 (Task type)
type Task =
    | Process(id: I32, data: Str)
    | Stop;

/// 生产者 (Producer)
fn producer(
    tx: Sender[Task],
    task_count: I32
)
{
    let tasks = (1..=task_count).map(|i| {
        Task.Process(i, f"Task data {i}")
    });

    tasks.for_each(|task| tx.send(task));
    tx.send(Task.Stop);  // 发送停止信号 (Send stop signal)
}

/// 消费者 (Consumer)
fn consumer(
    id: I32,
    rx: Receiver[Task],
    result_tx: Sender[Str]
)
{
    /// 递归处理消息 (Recursive message processing with TCO)
    fn process(id: I32, rx: Receiver[Task], result_tx: Sender[Str]) {
        match rx.recv() {
            Task.Process(task_id, data) => {
                // 模拟处理 (Simulate processing)
                let result = f"Consumer {id} processed task {task_id}: {data}";
                result_tx.send(result);
                process(id, rx, result_tx)  // 尾递归 (Tail recursion)
            },
            Task.Stop => {
                // 停止接收 (Stop receiving)
            },
        }
    }
    process(id, rx, result_tx)
}

/// 结果收集器 (Result collector)
fn collector(
    rx: Receiver[Str],
    expected_count: I32
)
{
    /// 递归收集结果 (Recursive result collection with TCO)
    fn collect(rx: Receiver[Str], remaining: I32) {
        if remaining <= 0 {
            return;
        }
        let result = rx.recv();
        print(result);
        collect(rx, remaining - 1)  // 尾递归 (Tail recursion)
    }
    collect(rx, expected_count)
}

/// 主函数 (Main function)
fn main() {
    let task_count = 10;
    let consumer_count = 3;

    let (task_tx, task_rx) = Channel.new[Task](buffer: 5);
    let (result_tx, result_rx) = Channel.new[Str](buffer: 10);

    parallel_scope {
        // 启动生产者 (Start producer)
        spawn {
            producer(task_tx, task_count);
        };

        // 启动多个消费者 (Start multiple consumers)
        (1..=consumer_count).for_each(|i| {
            let rx_clone = task_rx.clone();
            let tx_clone = result_tx.clone();
            spawn {
                consumer(i, rx_clone, tx_clone);
            };
        });

        // 启动结果收集器 (Start result collector)
        spawn {
            collector(result_rx, task_count);
        };
    }

    print("All tasks completed!");
}
```

---

## 14. 附录 (Appendix)

### 14.1 关键字完整表 (Complete keyword table)

| 关键字 | 用途 |
|--------|------|
| `fn` | 函数定义 (Function definition) |
| `let` | 不可变绑定 (Immutable binding) |
| `if` / `else` | 条件表达式 (Conditional expression) |
| `match` | 模式匹配 (Pattern matching) |
| `struct` | 结构体定义 (Struct definition) |
| `type` | 类型别名/枚举定义 (Type alias/enum definition) |
| `capability` | Trait 定义 (Trait definition) |
| `impl` | Trait 实现块 (Trait implementation block) |
| `deriving` | 自动派生声明 (Auto-derive declaration) |
| `pub` / `pub(pkg)` | 可见性修饰符 (Visibility modifiers) |
| `import` | 导入模块路径 (Import module path) |
| `alias` | 类型/项别名 (Type/item alias) |
| `where` | 泛型类型约束 (Generic type constraints) |
| `with` | （已移除）属性由编译器从 `uses` 集合自动推断 (Removed - properties auto-inferred from `uses` set) |
| `uses` | 依赖声明 (Dependency declaration) |
| `effects` | 效应声明（保留关键字）(Effect declaration - reserved) |
| `cost` | 成本约束 (Cost constraint) |
| `spawn` | 生成并发任务 (Spawn concurrent task) |
| `select` | 多路复用 (Multiplex channels) |
| `parallel_scope` | 并发作用域 (Parallel scope) |
| `const` | 常量定义 (Constant definition) |
| `return` | 提前返回 (Early return) |
| `throw` | 抛出错误 (Throw error) |

### 14.2 操作符优先级 (Operator precedence)

从高到低 (Highest to lowest):

1. 字段访问、方法调用 (Field access, method call): `.`, `()`
2. 一元操作符 (Unary): `-`, `!`, `~`
3. 乘除模 (Multiplication, division, modulo): `*`, `/`, `%`
4. 加减 (Addition, subtraction): `+`, `-`
5. 位移 (Bit shift): `<<`, `>>`
6. 按位与 (Bitwise AND): `&`
7. 按位异或 (Bitwise XOR): `^`
8. 按位或 (Bitwise OR): `|`
9. 比较 (Comparison): `==`, `!=`, `<`, `>`, `<=`, `>=`
10. 逻辑与 (Logical AND): `&&`
11. 逻辑或 (Logical OR): `||`
12. 范围 (Range): `..`, `..=`
13. 管道 (Pipe): `|>`
14. 错误传播 (Error propagation): `?`
15. 赋值 (Assignment): `=`

### 14.3 类型系统速览 (Type system overview)

#### 基本类型 (Primitive types)

```spore
I32, I64      // 有符号整数 (Signed integers)
U32, U64      // 无符号整数 (Unsigned integers)
F32, F64      // 浮点数 (Floating point)
Bool          // 布尔 (Boolean)
Char          // Unicode 标量值 (Unicode scalar value)
Str           // UTF-8 字符串 (UTF-8 string)
()            // unit 类型 (unit type)
```

#### 集合类型 (Collection types)

```spore
List[T]       // 列表 (List)
Map[K, V]     // 映射 (Map)
Set[T]        // 集合 (Set)
Array[T, N]   // 固定大小数组 (Fixed-size array)
```

#### 特殊类型 (Special types)

```spore
Option[T]     // 可选值 (Optional value)
Result[T, E]  // 结果（成功/错误）(Result - success/error)
Ref[T]        // 可变引用容器 (Mutable reference container)
Channel[T]    // 并发通道 (Concurrent channel)
```

### 14.4 语法 EBNF 概要 (EBNF grammar sketch)

```ebnf
Program       = { ImportDecl | AliasDecl | Function | Struct | Type | Capability }
ImportDecl    = "import" ModulePath [ "as" Ident ]
AliasDecl     = "alias" Ident "=" QualifiedItem
Function      = "fn" Ident [ TypeParams ] "(" [ Params ] ")" [ "->" Type ]
                [ "!" "[" Types "]" ] [ WhereClause ] [ UsesClause ] [ CostClause ] [ SpecClause ] Block
CostClause    = "cost" "[" CostSlot "," CostSlot "," CostSlot "," CostSlot "]"
CostSlot      = IntLiteral | ParamVar | "O" "(" ParamVar ")"
ParamVar      = Ident   -- must name a function parameter
Struct        = "struct" Ident [ TypeParams ] StructBody [ "deriving" "[" Capabilities "]" ]
Type          = "type" Ident [ TypeParams ] "=" TypeDef
Capability    = "capability" Ident [ TypeParams ] "{" { CapabilityItem } "}"
ModulePath    = Ident { "." Ident }
QualifiedItem = ModulePath "." Ident

Expr          = Literal | Ident | Block | If | Match | Lambda | BinOp | UnaryOp | Call | Pipe
Block         = "{" { Stmt ";" } [ Expr ] "}"
If            = "if" Expr Block "else" ( If | Block )
Match         = "match" Expr "{" { Pattern "=>" Expr "," } "}"
Lambda        = "|" [ Params ] "|" ( Expr | Block )
Pipe          = Expr "|>" Expr

Pattern       = Literal | Ident | Constructor | Struct | List | "_" | Pattern "|" Pattern
Constructor   = Ident "(" [ Pattern { "," Pattern } ] ")"
SpecClause    = "spec" "{" { SpecItem } "}"

Type          = Ident | Type "[" Types "]" | "(" [ Types ] ")" "->" Type [ "!" "[" Types "]" ]
```

> 注：以上 `Function` 产生式按文档推荐顺序书写签名子句；解析器实际接受 `where`、`uses`、`cost`、`spec` 按任意顺序出现并进行规范化。当前 `CostSlot` 仅覆盖整数常量、参数变量与线性 `O(n)` 记法；`urls.len`、`expr_size(expr) * 10`、`O(log n)`、`max`/`min` 等更丰富形式仍待后续版本统一。`Module` 产生式在本批次保持原样，模块块语法仍待后续批次统一。

### 14.5 设计决策总结 (Design decisions summary)

1. **表达式优先**：减少语句形式，增强可组合性
2. **花括号**：明确作用域，易于解析
3. **分号语义**：Rust 启发，自然区分语句与表达式
4. **管道操作符**：提升数据流可读性，函数式风格
5. **固定操作符**：避免操作符重载的复杂性
6. **f-string/t-string**：现代字符串处理，灵活插值
7. **`! [Errors]`**：显式错误声明，类型安全的异常
8. **Lambda `|x| x`**：简洁闭包语法，高阶函数友好
9. **嵌套块注释**：更好的代码注释体验
10. **`let` 不可变 + `Ref[T]`**：默认不可变，显式可变性
11. **`match` 穷尽**：消除遗漏分支的 bug
12. **无循环**：鼓励递归 + TCO，函数式纯度
13. **`if` 表达式**：统一控制流与表达式
14. **后缀类型**：`name: Type` 与主流语言一致
15. **`deriving` + `impl`**：自动派生常见 trait，手动实现用 `impl` 块
16. **`struct`/`type`**：清晰区分记录与求和类型
17. **`capability` 关键字**：统一 trait 与能力系统
18. **TCO 保证**：递归无忧
19. **基本类型清单**：覆盖常见数值、字符串、集合

---

## 结语 (Conclusion)

本文档定义了 Spore 语言 v0.1 的核心语法规范，涵盖：

- **词法结构**：关键字、操作符、字面量、注释、标识符
- **类型系统**：struct、type、capability、泛型、refinement 类型
- **表达式系统**：if、match、lambda、pipe、block、error propagation
- **函数定义**：完整签名、where/uses/cost/spec 子句、效应、成本、资源依赖
- **模式匹配**：穷尽性、守卫、或模式、嵌套模式
- **模块系统**：可见性、导入、别名
- **并发机制**：parallel_scope、spawn、task.await、Channel.new、select
- **错误处理**：`! [Errors]`、`?` 操作符、match 错误
- **Hole 语法**：渐进式开发、类型推断
- **语法糖**：字符串插值、字段省略、管道变换
- **命名约定**：snake_case、PascalCase、SCREAMING_SNAKE_CASE

该规范为 Spore 编译器实现、IDE 工具支持、语言教学和开发者使用提供权威参考。

**版本历史**:
- v0.1 (2024): 初始规范，定义核心语法与设计决策
- v0.1.1 (2025): 合并 signature-syntax-v0.2 内容为附录 B

**下一步**:
- 补充标准库 API 规范
- 细化效应系统的语义定义
- 完善成本模型的形式化描述
- 添加更多实际项目示例

---

## 附录 B: 签名语法详解 (Signature Details)

> 本节内容整合自 `signature-syntax-v0.2.md`，提供函数签名的完整规范与示例。
> §5（函数定义）定义了签名的基本结构，本附录补充设计原则、效果推断规则、
> Snapshot Hash 覆盖范围等细节。

### B.1 设计原则

1. 错误是特殊返回类型 → 紧跟 `->` 用 `!` 分隔
2. 泛型约束 → `where`（Rust 风格）；资源 → `uses`；代价 → `cost`，各自独立子句；效果属性由编译器从 `uses` 自动推断
3. 简单纯函数零开销 → 无 where、无 `!`
4. 编译器推断并显示所有省略的元数据

### B.2 语法模板

```
fn <name>[<generics>](<params>) -> <ReturnType> [! [<ErrorTypes>]]
[where <GenericName>: <Constraint>]  -- repeat one line per bound
[uses [<Capability>, ...]]
[cost [<compute>, <alloc>, <io>, <parallel>]]
[spec { ... }]
{
    <body>
}
```

其中 `<compute>` / `<alloc>` / `<io>` / `<parallel>` 当前都只接受三类写法：整数常量、参数变量、线性 `O(n)`。

### B.3 签名子句排列顺序（规范约定）

解析器接受 `where`、`uses`、`cost`、`spec` 子句按任意顺序出现。语法标准不把子句顺序视为语义的一部分；为了保证文档、格式化输出与代码评审的一致性，推荐的规范顺序为：`where` → `uses` → `cost` → `spec`。当前规范中 `cost` 只接受固定顺序向量 `cost [compute, alloc, io, parallel]`，并且每个槽位只接受整数常量、参数变量或线性 `O(n)`；旧的 `cost <= expr`、`log/max/min` 风格的标量表面语法，以及更丰富的代数/组合项都留待后续版本讨论。

```spore
-- Canonical order
where T: Serialize
where T: Eq
where U: Display
uses [Compute]
cost [500, 0, 0, 0]
spec {
    example "round-trip" => encode(value) |> decode == value
}
```

编译器格式化输出与文档示例都遵循这一顺序。`where T: Serialize + Eq`、`where { ... }` 与逗号分组写法仍然属于未来讨论，不是当前规范的一部分。

### B.4 效果属性（编译器自动推断）

编译器根据 `uses` 声明自动推断以下属性，无需手动标注：

| 属性 | 推断规则 |
|------|---------|
| `pure` | `uses []` → 自动推断为 pure |
| `deterministic` | `uses` 中不含 Random/Clock → 自动推断 |
| `total` | 编译器验证终止性 → 自动推断 |

`idempotent` 无法从 `uses` 自动推断，需通过文档注释标注：`/// @idempotent`

蕴含关系：`pure` ⊃ `deterministic`（pure 必然 deterministic）

### B.5 Snapshot Hash 覆盖范围

以下任一变更 → 新 hash → 需要 `--permit`：

| 签名组件 | 示例变更 |
|----------|---------|
| 函数名 | `parse_config` → `load_config` |
| 参数名 | `raw` → `input` |
| 参数顺序 | `(a, b)` → `(b, a)` |
| 参数类型 | `Str` → `Bytes` |
| 返回类型 | `Config` → `Settings` |
| 错误类型集合 | 增删任一错误类型 |
| 代价上界 | `≤ 200` → `≤ 300` |
| 能力集 | 增删任一能力 |
| 泛型约束 | `T: Eq` → `T: Eq + Hash` |

### B.6 补充示例

#### 编译器推断输出示例

```spore
fn add(a: I32, b: I32) -> I32 {
    a + b
}
```

编译器推断输出：
```
  cost [1, 0, 0, 0]
  uses []
  -- 编译器自动推断: pure, deterministic, total (基于 uses [])
```

#### 有错误的纯函数

```spore
fn parse_int(input: Str) -> I32 ! [InvalidFormat] {
    ...
}
```

编译器推断输出：
```
  cost [12, 0, 0, 0]
  uses [Compute]
  -- 编译器自动推断: deterministic (基于 uses [Compute])
```

#### 不完整函数（未声明 uses，有能力依赖）

```spore
fn fetch_data(url: Url) -> Data ! [NetworkError] {
    http.get(url)    -- 调用了 http 模块
}
```

编译器输出：
```
ERROR [incomplete-function] fetch_data 是不完整函数：
  检测到能力依赖但未声明 `uses`。
  推断能力集: [NetRead]

  建议添加:
    uses [NetRead]

  当前状态: 可模拟执行，不可真实执行
```

#### Hole（部分定义，可模拟执行）

```spore
/// @idempotent
fn validate_payment(amount: Money, method: PaymentMethod) -> Receipt ! [Declined, InsufficientFunds]
uses [NetRead]
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
  "available_capabilities": ["NetRead"],
  "candidate_functions": [
    "payment_gateway.charge(amount: Money, method: PaymentMethod) -> Receipt ! [Declined, InsufficientFunds]"
  ],
  "error_types_to_handle": ["Declined", "InsufficientFunds"]
}
```

---

**文档维护**: Spore Language Team
**许可证**: MIT License
**反馈**: https://github.com/spore-lang/spore/issues
