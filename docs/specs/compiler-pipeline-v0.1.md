# Spore 编译器 Pipeline 架构 v0.1

## 总览

```
Source Text
    │
    ▼
┌─────────┐
│  Lexer  │ → Token Stream (with Span)
└─────────┘
    │
    ▼
┌─────────┐
│ Parser  │ → AST (Untyped, full Span, preserves sugar)
└─────────┘
    │
    ▼
┌────────────────┐
│ Resolve+Desugar│ → HIR (desugared, names resolved)
└────────────────┘   sig hash computed here
    │
    ▼
┌──────────────────────────┐
│ TypeCheck+CapCheck+Cost  │ → TypedHIR (fully typed, verified)
└──────────────────────────┘   impl hash computed here
    │
    ▼
┌─────────┐
│ Codegen │ → Cranelift IR → Native Code
└─────────┘
```

## IR 层次

### 1. AST (Abstract Syntax Tree)

**职责**: 忠实反映源码结构

- 与源码 1:1 对应
- 所有节点带 `Span`（文件 + 字节偏移范围）
- 保留所有语法糖: `|>`, `?`, `f"..."`, `t"..."`
- 用途: 错误报告精确指向源码位置、IDE 语法高亮、代码格式化

**数据结构要点**:
```
Spanned<T> { node: T, span: Span }
Span { file: FileId, start: u32, end: u32 }

Module { items: Vec<Spanned<Item>> }
Item = FnDef | StructDef | TypeDef | CapabilityDef | Import | ...
FnDef { name, params, return_type, error_set, where_clause, with_clause, cost_clause, uses_clause, body }
Expr = Literal | Ident | BinOp | UnaryOp | Call | Pipe | TryOp | FString | Match | If | Lambda | Block | Hole | ...
```

### 2. HIR (High-level IR)

**职责**: 简化后的规范表示，名称全部解析

**Resolve pass**:
- 名称解析: 每个标识符绑定到唯一声明（DefId）
- 导入解析: 模块路径 → 具体模块引用
- 可见性检查: pub / pub(pkg) / private
- 循环依赖检测
- Hole 记录: 标记 `?name` 位置及当前作用域

**Desugar pass**（与 Resolve 同层）:
- `a |> f(b)` → `f(a, b)`
- `expr?` → `match expr { Ok(v) => v, Err(e) => return Err(e.into()) }`
- `f"hello {name}"` → `format("hello {}", name)`
- `t"hello {name}"` → `Template([Literal("hello "), Interpolation(name)])`

**sig hash 在此层计算**:
- 覆盖: 函数名、参数类型、返回类型、错误集、效果声明、能力声明、代价声明
- 不覆盖: 函数体
- 用途: sig hash 不变 → 下游依赖模块跳过 resolve + type_check

### 3. TypedHIR (Typed High-level IR)

**职责**: 完全类型化、能力/代价验证后的 IR

**TypeCheck**（统一 pass，包含能力和代价检查）:
- 双向类型推断（bidirectional type inference）
- 签名必须完整注解，函数体内推断
- Trait/Capability 解析
- 关联类型 + GAT 实例化
- Const generics 求值
- 穷尽性检查（match 表达式）
- 错误集传播与一致性验证（`! [Errors]`）
- 细化类型: L0 可判定谓词 + L1 抽象解释传播

**CapCheck**（合并在 TypeCheck 中）:
- 函数体能力使用 ⊆ 声明能力集
- Capability = Trait，与类型解析共享基础设施
- 模块能力封顶检查

**CostCheck**（合并在 TypeCheck 中）:
- 抽象解释计算四维代价: compute(op) + alloc(cell) + io(call) + parallel(lane)
- 验证 ≤ 声明上界
- 符号代价表达式支持
- unbounded 检测

**Hole 报告生成**:
- 完整上下文: 类型、可用绑定、能力集、代价预算、候选函数

**impl hash 在此层计算**:
- 覆盖: 类型检查通过后的完整 HIR hash
- 部分函数（含 hole）impl hash 为 None
- 用途: impl hash 不变 → 跳过 codegen

### 4. Codegen（TypedHIR → Cranelift IR → Native）

- 无独立 LIR，Cranelift IR 充当低级 IR
- Cranelift 函数级粒度，契合内容寻址
- TCO（尾调用优化）在此层实现
- 模式匹配降级为分支/跳转

## salsa 增量编译集成

```rust
#[salsa::input]
struct SourceFile {
    #[return_ref]
    path: PathBuf,
    #[return_ref]
    contents: String,
}

#[salsa::tracked]
fn lex(db: &dyn Db, file: SourceFile) -> TokenStream { ... }

#[salsa::tracked]
fn parse(db: &dyn Db, tokens: TokenStream) -> Ast { ... }

#[salsa::tracked]
fn resolve(db: &dyn Db, ast: Ast) -> Hir { ... }
// side product: sig_hash

#[salsa::tracked]
fn type_check(db: &dyn Db, hir: Hir) -> TypedHir { ... }
// side product: impl_hash, hole_reports, diagnostics

#[salsa::tracked]
fn codegen(db: &dyn Db, typed_hir: TypedHir) -> CompiledModule { ... }
```

**增量策略**:
1. 文件内容变更 → 重新 lex + parse（salsa 自动判断）
2. AST 变更但 sig hash 不变 → 下游模块跳过 resolve + type_check
3. sig hash 变更 → 触发下游 resolve + type_check（仅直接依赖）
4. impl hash 不变 → 跳过 codegen（Cranelift 缓存命中）

## 设计决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| IR 层数 | 3 层 (AST/HIR/TypedHIR) | 不需要 MIR（无 borrow checker），不需要 flat IR（无 comptime） |
| 独立 LIR | 不需要 | Cranelift IR 充当 LIR |
| 能力+代价检查位置 | 合并到 TypeCheck | capability = trait，与类型信息交叉使用 |
| 脱糖位置 | 全在 Resolve 层 | `\|>` `?` `f"..."` 均为纯语法变换 |
| sig hash 位置 | Resolve 层 | 签名信息在此层已完全确定 |
| impl hash 位置 | TypeCheck 后 | 需要完整类型检查通过 |
| Comptime | 不支持 | const generics + 细化类型 + 代价模型已覆盖；Elm/Roc/Gleam 均无 |
| flat IR (ZIR 风格) | 不需要 | 无 comptime，salsa 提供增量缓存 |

## 参考语言对比

| 语言 | Pipeline | Spore 借鉴 |
|------|----------|-----------|
| Rust | AST → HIR → THIR → MIR → LLVM IR | HIR/TypedHIR 概念，salsa 增量 |
| Zig | AST → ZIR → AIR → Machine IR | Pratt 解析器；不借鉴 ZIR（因无 comptime） |
| Roc | AST → Canonical → Solved → IR | Canonical ≈ HIR 概念，名称解析方式 |
| Gleam | Untyped AST → Typed AST | 简洁的 2 层方式参考 |
| Gonidium | AST → TypedDag | DiagCollector 错误收集模式 |
