# 语法规范决策记录

以下决策已最终确定，所有文档和实现必须遵守。主设计文档见 [`../DESIGN.md`](../DESIGN.md)，完整规范见 `spore-evolution/seps/SEP-0001-core-syntax.md`。

## 结构与关键字

| # | 决策 | 说明 |
|---|------|------|
| D1 | `struct` 用于积类型 | 不使用 `type = {}`，parser 仅支持 `struct` |
| D2 | `trait` 取代 `capability` | parser 直接拒绝旧关键字，无兼容重写 |
| D7 | 删除 `module` 关键字 | 模块名由文件路径推导 |
| N2 | `type Name { Variant(T) }` 用于枚举 | 花括号界定，位置参数字段 |
| N3 | 无 tuple structs | 仅支持 `struct Name { field: Type }` |
| N7 | `struct` = 积类型，`type` = 和类型 | `type` 关键字仅用于 enum/ADT |

## 签名子句

| # | 决策 | 说明 |
|---|------|------|
| D3 | spec-clause 作为签名子句 | `spec { example "...": expr }` |
| D11 | `! E1 \| E2` 错误集语法 | 管道符，无方括号，fn-def 与 type-expr 通用 |
| N1 | `cost [c, a, i, p]` 向量形式 | 固定顺序 compute/alloc/io/parallel；避免非 ASCII `≤` |
| N8 | `@unbounded` 仍必须写 `cost [...]` | 逃过体预算验证但保留期望代价向量；缺失时报 K0303 |
| N4 | `where` 子句不支持 `+` | 每个参数单一约束；`where T: Bound, U: Bound` |
| N5 | Spec 子句使用 `:` 分隔符 | `example "name": expr`，非 `=>` |

## 类型与谓词

| # | 决策 | 说明 |
|---|------|------|
| D6 | `when` 用于 refinement types | 避免与 `if` 表达式歧义 |
| D8 | `[T]` 用于泛型 | 避免 `<>` 解析歧义 |
| D9 | `Str` 为规范字符串类型名 | 与 `Int` / `Bool` / `Float` 一致 |
| D10 | `when self > 0` 谓词绑定 | 隐式 `self`，非 lambda |
| N6 | `Int` / `Float` 是 `I64` / `F64` 别名 | 尺寸类型是具体类型，抽象名是便利别名 |

## Effect 与错误

| # | 决策 | 说明 |
|---|------|------|
| D4 | `perform Effect.op(...)` 为 effect 调用语法 | 要求存在显式 `effect` 声明 |
| D5 | `throw expr` = `return Err(expr)` 语法糖 | 由 `?` 在调用边界受检传播 |

## 模块与导入

| # | 决策 | 说明 |
|---|------|------|
| D12 | 禁止选择性 / 通配符导入 | 仅 `import mod as alias`，见 SEP-0008 |

## 已知实现差距

| # | 决策 | 说明 |
|---|------|------|
| D13 | Range `a..b` | token 已词法化，尚无 parser 路径 |
