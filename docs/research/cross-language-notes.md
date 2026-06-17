# 跨语言调研沉淀

从 Agda / Idris / Unison / Elm / Roc / Rust / Zig / Gleam / Pharo 等语言调研中提取的、仍然影响 Spore 方向的持久结论。主设计文档见 [`../../SPARK.md`](../../SPARK.md)。

## 思想来源

| 语言   | Spore 采纳的核心思想                                   |
| ------ | ------------------------------------------------------ |
| Agda   | Holes 作为一等 typed placeholders                      |
| Idris  | Elaboration — 编译器填充程序员省略的细节               |
| Unison | 内容寻址代码 — 模块以 hash 而非 name+version 标识      |
| Elm    | 把人类可读的错误信息作为主设计目标                     |
| Roc    | Managed effects — 所有 IO 通过 Platform 提供的 handler |

## 方向性结论

| 主题         | 沉淀结论                                                                                                 |
| ------------ | -------------------------------------------------------------------------------------------------------- |
| 语法设计     | Expression-based 核心、有限关键字、无自定义操作符；签名显式承载错误 / effect / budget / properties       |
| 类型系统     | 函数边界显式、函数体内推断；穷尽匹配与错误信息质量是语言可用性的核心投资                                 |
| 依赖类型光谱 | 走 refinement + 抽象解释 + const generics 的 80/20 路线，而非 SMT / theorem proving / 全 dependent types |
| 模块系统     | 文件路径即模块名、private-by-default；避免独立 module language / functor，参数化优先用 generics + traits |
| 包管理       | 内容寻址 + 锁文件 + 哈希校验、去中心化或 Git-first 分发，优于 semver-first registry                      |
| 运行时       | Watch mode 目标是“保存后快速反馈”，不是 Erlang / Smalltalk 式运行时热替换                                |
| 实现技术栈   | Rust 在 ADT、增量编译、WASM、LSP、FFI、内容寻址与工具链成熟度上是最佳折中                                |
| 代码生成     | 先用 Cranelift 获得纯 Rust 实现、快编译与 WASM 友好性；LLVM 留作未来可选高性能后端                       |
| 解析器       | 手写递归下降 + Pratt（Rust / Zig / Roc / Unison / Elm / Gleam 全部手写）                                 |

## 非目标（明确排除）

- **HKT、全 dependent types、SMT 驱动证明、theorem proving**：不纳入语言目标
- **Borrow checker / lifetime 系统**：不引入；方向是 Perceus 风格 RC + region 优化
- **Mutation testing**：不进入近期关键路径；验证策略以 properties / refinement / evidence 为主线
- **运行时状态保持式热重载**：不承诺状态迁移、热升级、分布式热重载或动态装载协议
- **图灵完备的 comptime（Zig 风格）**：const generics + 细化类型 + 预算模型覆盖主要场景
- **独立 module language / functor / 模块级 effect carrier**：参数化 effect 交给 generics + traits + package / Platform 边界
- **模块级 effect ceiling**：effect checking 仅在函数级 `uses [...]` 与项目 / Platform 边界发生
- **自定义操作符**：管道 `|>` 是唯一特殊操作符
