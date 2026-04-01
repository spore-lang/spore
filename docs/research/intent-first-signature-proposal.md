# 意图优先的签名分层设计（提案 v0.1）

> 状态：Pre-SEP / 设计改进草案
>
> 本文档用于整理下一阶段的签名语法改进方向，作为后续 `spore-evolution` 仓库中正式 SEP 的种子文档。

## 背景

Spore 当前的签名设计已经具备很强的信息承载能力：

- 参数、返回类型、错误集
- `where` 泛型约束
- `uses` 能力集
- `cost` 代价上界
- hole / capability / platform 等配套系统

这一方向本身是成立的，但在“让人类与 Agent 更稳定地传达意图”这个目标上，现有设计仍有三个明显问题：

1. **主次不清**：主签名和约束元数据处于同一层级，阅读第一眼难以抓住核心意图。
2. **意图缺位**：函数“为什么存在、想达成什么、什么结果算合格”大多依赖命名、注释或外部文档表达。
3. **hole 协议不够语言内建**：`?name` 本身简洁，但 hole 的目标、约束、允许策略和验收条件主要依赖编译器外部协议补充。

因此，这份提案希望在不破坏 Spore 现有表达式风格、能力系统和 hole 思想的前提下，把“意图”提升为一等结构，并让签名信息分层显示。

## 设计目标

### 目标

1. 让函数的**主意图**在第一眼即可读。
2. 让 Agent 能稳定提取函数的**目标、约束、验收条件**。
3. 让 hole 从“占位语法”升级为“带协议的协作点”。
4. 保持 Spore 既有的 expression-based 风格和类型/能力/代价设计方向。

### 非目标

1. 不在本提案中重做模块系统或包管理。
2. 不在本提案中重新设计 effect/platform 的整体模型。
3. 不要求所有约束都必须在第一阶段可验证；允许语法先稳定、验证逐步补齐。

## 核心提案

### 1. 采用“主签名 + contract 块”的分层结构

主签名只保留一等 API 信息：

- 参数
- 返回值
- 错误集
- 必要时保留 `where` 泛型约束

其余语义与运行约束下沉到 `contract` 块：

- `intent`
- `requires`
- `ensures`
- `uses`
- `cost`
- `hole`
- `example`

模板如下：

```spore
fn <name>[<generics>](<params>) -> <ReturnType> [! [<ErrorTypes>]]
[where <GenericName>: <Constraint>, ...]
contract {
    [intent "<natural-language-intent>"]
    [requires <predicate>]
    [ensures <predicate>]
    [uses [<Capability>, ...]]
    [cost <= <CostExpr>]
    [hole <name>: <Type> { ... }]
    [example { ... }]
}
{
    <body>
}
```

### 2. `intent` 成为一等语法

`intent` 用于表达函数的目标与期望行为，不承担类型级证明职责，但会成为：

- 人类阅读时的主语义锚点
- Agent 生成/补全时的目标描述
- 诊断系统解释“声明意图与实现偏差”的依据

示例：

```spore
contract {
    intent "从文章正文提取忠实、简洁、可审查的摘要"
}
```

### 3. 引入 `requires` / `ensures`

在类型、错误和能力之外，签名应能显式表达行为层面的前置/后置条件。

```spore
contract {
    requires limit > 0
    ensures result.sentences.len <= limit
    ensures result.source == self.url
}
```

`requires` / `ensures` 的目标不是立刻变成完整定理证明系统，而是先为：

- 诊断
- hole 验收
- 测试生成
- Agent 候选实现排序

提供稳定语义接口。

### 4. hole 采用“声明 + 使用”双层结构

函数体内仍保留简洁的 `?draft` 用法，但其协议在 `contract` 中定义：

```spore
contract {
    hole draft: Summary {
        intent "先生成摘要初稿，再由实现决定是否直接采用"
        allows [split_sentences, rank_sentences, compress_sentences]
        accept result.source == self.url
        accept result.sentences.len <= limit
        accept faithful_to(result, self.body)
    }
}
```

设计原则：

- 函数体中的 `?draft` 保持轻量
- hole 的可用策略、上下文限制、验收条件语言内建
- 编译器可稳定导出结构化 `HoleReport`

### 5. 示例与诊断契约内建

为支持人类和 Agent 对齐，`contract` 允许携带少量可执行示例：

```spore
contract {
    example {
        summarize(article, 2).sentences.len <= 2
    }
}
```

`example` 不等价于完整测试框架，但它可以：

- 为 Agent 提供最小行为样例
- 为诊断提供更具上下文的解释
- 成为未来文档与测试生成的输入

## 完整示例

下面给出一份覆盖主要语法特性的完整示例：

```spore
module news.digest

import spore.net.fetch_text
import spore.time.now
import project.platform.NetRead as Http

pub type FetchError {
    Timeout,
    Parse(ParseError),
    EmptyBody,
}

pub struct Article {
    url: Url,
    title: Str,
    body: Str,
    fetched_at: Time,
}

pub struct Summary {
    source: Url,
    sentences: List[Str],
}

pub capability Summarizer[T] {
    fn summarize(self: T, limit: Int) -> Summary ! [FetchError]
    contract {
        intent "生成忠实、简洁、可审查的摘要"
        ensures result.sentences.len <= limit
    }
}

fn pick_top[T](xs: List[T], score: Fn(T) -> Int, limit: Int) -> List[T]
where T: Clone
contract {
    intent "按评分选出前 limit 个元素"
    cost <= O(xs.len * log(xs.len))
    requires limit >= 0
    ensures result.len <= limit
}
{
    xs
    |> sort_desc_by(|x| score(x))
    |> take(limit)
}

impl Summarizer[Article] for Article {
    fn summarize(self: Article, limit: Int) -> Summary ! [FetchError]
    contract {
        intent "从文章正文提取不超过 limit 句的忠实摘要"

        uses [Http, Spawn]
        cost <= O(self.body.len + limit)

        requires limit > 0
        ensures result.source == self.url
        ensures result.sentences.len <= limit

        hole draft: Summary {
            intent "先生成摘要初稿，再由实现决定是否直接采用"
            allows [split_sentences, rank_sentences, compress_sentences]
            accept result.source == self.url
            accept result.sentences.len <= limit
            accept faithful_to(result, self.body)
        }

        example {
            summarize(
                Article {
                    url: "https://example.com",
                    title: "Hello",
                    body: "A. B. C.",
                    fetched_at: now(),
                },
                2,
            ).sentences.len <= 2
        }
    }
    {
        let title_task = spawn { fetch_title(self.url)? };
        let body_task = spawn { fetch_text(self.url)? };

        let article = Article {
            url: self.url,
            title: await title_task,
            body: await body_task,
            fetched_at: now(),
        };

        if article.body.len == 0 {
            throw FetchError.EmptyBody
        }

        let ranked =
            article.body
            |> split_sentences
            |> map(|s| rank_sentence(article.title, s));

        let draft = ?draft;

        match draft {
            Summary { sentences, .. } if sentences.len > 0 => draft,
            _ => Summary {
                source: article.url,
                sentences: pick_top(ranked, |x| x.score, limit)
                    |> map(|x| x.text),
            },
        }
    }
}
```

## 与当前设计的对比

### 当前设计（简化示意）

```spore
fn summarize(self: Article, limit: Int) -> Summary ! [FetchError]
where T: Bound
uses [Http, Spawn]
cost ≤ O(n)
{
    let draft = ?draft : Summary;
    ...
}
```

### 提案设计（分层版）

```spore
fn summarize(self: Article, limit: Int) -> Summary ! [FetchError]
where T: Bound
contract {
    intent "从文章正文提取不超过 limit 句的忠实摘要"
    uses [Http, Spawn]
    cost <= O(n)
    requires limit > 0
    ensures result.sentences.len <= limit
    hole draft: Summary {
        allows [split_sentences, rank_sentences]
        accept result.sentences.len <= limit
    }
}
{
    let draft = ?draft;
    ...
}
```

### 改动摘要

1. **平铺签名改为分层签名**
   `uses` / `cost` 等约束信息不再与主签名并列堆叠，而进入 `contract`。

2. **新增 `intent`**
   让“函数想完成什么”成为显式结构，而不是依赖注释和命名推断。

3. **新增 `requires` / `ensures`**
   让行为层约束语言内建，便于诊断、测试和 Agent 约束提取。

4. **新增 hole 契约声明**
   保留 `?name` 的简洁使用方式，同时让 hole 的可用策略和验收条件一等化。

5. **建议采用显式 `impl` 块**
   使“谁为谁实现 capability”对人类与工具都更清晰。

## 设计收益

### 对人类

1. **第一眼更聚焦**：主签名突出输入/输出/错误，而不是把所有信息摊平。
2. **渐进展开**：只有在需要时才进入 `contract` 查看更细的语义与约束。
3. **意图可见**：阅读代码时不再只能从命名猜测函数目标。

### 对 Agent

1. **可稳定提取目标**：`intent` 提供直接目标描述。
2. **可稳定提取约束**：`requires` / `ensures` / `accept` 可映射到结构化 IR。
3. **hole 协议更强**：Agent 可在语言层获得候选策略与验收条件，而非只看到一个裸洞。

## 对编译器与工具链的要求

若未来接受这套设计，编译器应提供对应的结构化导出表示，例如：

- Signature IR
- Contract IR
- Hole Contract IR
- Diagnostics with intent/contract references

建议 JSON/IR 至少包括：

- 函数主签名
- `intent`
- `requires`
- `ensures`
- `uses`
- `cost`
- hole 声明与 `accept` 条件
- `example`

## 兼容性与迁移

### 兼容方向

可提供一个机械迁移路径：

```spore
fn f(...) -> T
uses [...]
cost ≤ N
{
    ...
}
```

自动改写为：

```spore
fn f(...) -> T
contract {
    uses [...]
    cost <= N
}
{
    ...
}
```

### 迁移原则

1. 旧语法可短期兼容并由格式化器迁移。
2. `contract` 是语义分层，不应改变原有运行语义。
3. `intent` / `requires` / `ensures` / `example` 可先增量落地，不强制一次到位。

## 未决问题

1. `where` 是否保留在主签名层，还是也进入 `contract`？
2. `example` 是纯文档语义、编译期检查，还是可选测试输入？
3. `accept` 与 `ensures` 的关系如何定义：hole 局部契约是否是函数后置条件的子集？
4. `impl` 是否应同时成为正式语法调整的一部分，还是单独提案？

## 建议的后续步骤

1. 在 `spore-evolution` 仓库中把本文收敛为正式 SEP。
2. 为 `contract`、`intent`、`requires`、`ensures`、`hole` 块补充 EBNF。
3. 定义最小结构化 IR / JSON 导出格式。
4. 用 3 到 5 个真实案例验证：
   - 仅人类阅读是否更清晰
   - Agent 是否能更稳定地补全 hole
   - 诊断是否能基于意图生成更可解释的信息
