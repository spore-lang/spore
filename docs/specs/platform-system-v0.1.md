# Spore Platform System 规范 v0.1

**版本**: 0.1  
**日期**: 2025-01-03  
**状态**: Draft

---

## 1. 概述

### 1.1 什么是 Platform

Platform 是 Spore 语言中负责**执行所有 IO 操作**的运行时环境。Spore 采用彻底的纯函数式设计：应用代码**永远不直接执行 IO**，而是发出 effect（效应），Platform 提供 effect handler 在运行时解释这些 effect。

```spore
// 应用代码：纯函数，只发出 effect
fn read_config() -> Config ! [FileRead] {
    let content = File.read("config.toml")  // 发出 FileRead effect
    parse_toml(content)
}

// Platform：提供 FileRead 的 handler
// 应用代码看不到这个实现
handler FileReadHandler {
    File.read(path) -> resume(os_read_file(path))
}
```

**核心原则**:
- **应用代码 = 纯函数**：不包含任何 IO 实现，完全可测试
- **Platform = Effect 解释器**：将抽象的 effect 映射到具体的系统调用
- **显式契约**：Platform 明确声明它处理哪些 capability，编译器验证覆盖性

### 1.2 为什么通过 Platform 实现所有 IO

传统语言中，IO 操作直接内置在标准库或语言运行时中，导致：

1. **测试困难**：IO 代码必须 mock 或在真实环境中运行
2. **平台绑定**：应用代码直接依赖操作系统 API
3. **不可替换**：无法为不同环境（CLI/Web/Embedded）提供不同 IO 实现

Spore 的 Platform 模型解决了这些问题：

```spore
// 相同的应用代码
fn main(args: List[Str]) -> I32 ! [FileRead, StdOut] {
    let content = File.read("data.txt")
    println(content)
    0
}

// 在 CLI Platform：File.read → 调用 POSIX read()
// 在 WASM Platform：File.read → 调用 WASI fd_read()
// 在 Test Platform：File.read → 返回 mock 数据
```

### 1.3 与其他方案的对比

| 方案 | IO 模型 | 可测试性 | 平台独立性 | 例子 |
|------|---------|----------|------------|------|
| **传统语言** | 内置 IO | 需要 mock | 低 | C, Go, Java |
| **Monadic IO** | IO Monad | 中等 | 低 | Haskell |
| **Effect System** | Algebraic Effects | 高 | 中等 | Koka, Eff |
| **Spore Platform** | Effect Handlers + Platform | **极高** | **极高** | Roc, Spore |

Spore 与 Roc 的设计哲学一致：**没有内置 Platform**，所有 Platform 都是第三方包。

---

## 2. Platform 契约

### 2.1 Platform 必须提供的内容

一个完整的 Platform 必须声明：

1. **Capability 集合**：这个 Platform 处理哪些 IO capability
2. **Effect Handler 实现**：每个 capability 对应的运行时处理逻辑
3. **Entry Point 类型**：应用程序的入口函数必须满足的类型签名

```spore
// Platform 定义示例
platform CliPlatform {
    // 1. 声明处理的 capability
    handles [FileRead, FileWrite, NetRead, NetWrite, Clock, Spawn, Exit]
    
    // 2. Entry point 类型约束
    entry: fn(args: List[Str]) -> I32 ! [Exit]
    
    // 3. Handler 实现（由 Platform 提供）
    handler FileReadHandler { ... }
    handler FileWriteHandler { ... }
    // ...
}
```

### 2.2 示例：CLI Platform

CLI Platform 提供标准命令行应用所需的所有 capability：

```spore
platform CliPlatform {
    version: "1.0.0"
    
    handles [
        FileRead,      // 读文件
        FileWrite,     // 写文件
        NetRead,       // 网络读取
        NetWrite,      // 网络写入
        Clock,         // 获取时间
        Spawn,         // 并发（spawn task）
        StdOut,        // 标准输出
        StdErr,        // 标准错误
        Exit,          // 退出程序
    ]
    
    entry: fn(args: List[Str]) -> I32 ! [Exit]
    
    // Platform 实现细节（应用代码看不到）
    handler FileReadHandler {
        File.read(path: Path) -> Result[Bytes, IoError] {
            // FFI 调用 Rust/C 实现
            resume(ffi_read_file(path))
        }
        
        File.read_to_string(path: Path) -> Result[Str, IoError] {
            let bytes = File.read(path)?
            Str.from_utf8(bytes)
        }
    }
    
    handler SpawnHandler {
        Task.spawn<T>(f: fn() -> T) -> Task[T] {
            // 使用 Platform 的 runtime（如 tokio）
            resume(runtime_spawn(f))
        }
        
        Task.await<T>(task: Task[T]) -> T {
            resume(runtime_await(task))
        }
    }
}
```

### 2.3 示例：Web Platform

Web Platform 用于构建 HTTP 服务，提供不同的 capability 集合：

```spore
platform WebPlatform {
    version: "1.0.0"
    
    handles [
        HttpServer,    // HTTP 服务器
        NetRead,
        NetWrite,
        Clock,
        Spawn,
        DbQuery,       // 数据库查询
    ]
    
    // Entry point 是一个 HTTP handler
    entry: fn(req: Request) -> Response ! [HttpServer, DbQuery]
    
    handler HttpServerHandler {
        Server.listen(port: U16, handler: fn(Request) -> Response) {
            resume(ffi_listen_http(port, handler))
        }
    }
}
```

### 2.4 示例：Lambda Platform

Lambda Platform 用于 AWS Lambda 函数：

```spore
platform LambdaRlatform {
    version: "1.0.0"
    
    handles [
        NetRead,
        NetWrite,
        S3Read,        // S3 操作
        S3Write,
        DynamoRead,    // DynamoDB 操作
        DynamoWrite,
    ]
    
    // Lambda 的 entry point
    entry: fn(event: JsonValue) -> JsonValue ! [S3Read, DynamoWrite]
    
    handler S3Handler {
        S3.get(bucket: Str, key: Str) -> Result[Bytes, S3Error] {
            resume(aws_s3_get(bucket, key))
        }
    }
}
```

---

## 3. Platform 定义语法

### 3.1 Platform 声明

Platform 是一种特殊的 Spore 包，使用 `platform` 关键字定义：

```spore
// file: platform.spore
platform MyPlatform {
    // 版本信息
    version: "1.0.0"
    
    // 声明处理的 capability
    handles [Cap1, Cap2, Cap3]
    
    // Entry point 类型
    entry: fn(Input) -> Output ! [Effects]
    
    // 可选：平台特定的配置
    config: {
        max_threads: U32,
        buffer_size: U64,
    }
}
```

### 3.2 Handler 实现

每个 capability 都需要一个对应的 handler：

```spore
// FileRead capability 的 handler
handler FileReadHandler {
    // 处理 File.read effect
    File.read(path: Path) -> Result[Bytes, IoError] {
        // resume(value) 将结果返回给应用代码
        resume(native_read_file(path))
    }
    
    // 可以有多个相关的 effect
    File.exists(path: Path) -> Bool {
        resume(native_file_exists(path))
    }
    
    File.list_dir(path: Path) -> Result[List[Path], IoError] {
        resume(native_list_dir(path))
    }
}
```

### 3.3 Effect Handler 模式

Handler 可以：

1. **直接返回结果**：调用 `resume(value)`
2. **触发其他 effect**：handler 内部可以使用其他 capability
3. **转换 effect**：将高级 effect 转换为低级 effect

```spore
// 示例：将 HttpClient effect 转换为 NetRead/NetWrite
handler HttpClientHandler uses [NetRead, NetWrite] {
    Http.get(url: Url) -> Result[Response, HttpError] {
        // 1. 建立 TCP 连接（发出 NetRead/NetWrite effect）
        let socket = TcpSocket.connect(url.host, url.port)?
        
        // 2. 发送 HTTP 请求
        socket.write(format_http_request(url))?
        
        // 3. 读取响应
        let response = socket.read_until_eof()?
        
        // 4. 返回给应用代码
        resume(parse_http_response(response))
    }
}
```

### 3.4 FFI 集成

Platform 的底层实现通常使用 native 代码（Rust/C）：

```spore
// Platform 中的 FFI 声明
foreign fn ffi_read_file(path: Bytes) -> FfiResult[Bytes]
foreign fn ffi_write_file(path: Bytes, content: Bytes) -> FfiResult[Unit]
foreign fn ffi_tcp_connect(host: Bytes, port: U16) -> FfiResult[FfiSocket]

handler FileWriteHandler {
    File.write(path: Path, content: Bytes) -> Result[Unit, IoError] {
        let result = ffi_write_file(path.to_bytes(), content)
        match result {
            FfiOk(()) -> resume(Ok(()))
            FfiErr(code) -> resume(Err(IoError.from_code(code)))
        }
    }
}
```

对应的 Rust 实现：

```rust
// platform_ffi.rs
#[no_mangle]
pub extern "C" fn ffi_read_file(path: SporeBytes) -> FfiResult<SporeBytes> {
    let path_str = unsafe { path.to_str() };
    match std::fs::read(path_str) {
        Ok(bytes) => FfiResult::ok(SporeBytes::from_vec(bytes)),
        Err(e) => FfiResult::err(e.raw_os_error().unwrap_or(-1)),
    }
}
```

### 3.5 完整 Platform 示例

```spore
// file: platform.spore
platform SimplePlatform {
    version: "1.0.0"
    
    handles [FileRead, StdOut, Exit]
    
    entry: fn(args: List[Str]) -> I32 ! [Exit]
}

// file: handlers/file.spore
handler FileReadHandler {
    File.read(path: Path) -> Result[Bytes, IoError] {
        resume(ffi_read_file(path.to_bytes()))
    }
}

// file: handlers/stdout.spore
handler StdOutHandler {
    println(s: Str) -> Unit {
        resume(ffi_println(s.to_bytes()))
    }
}

// file: handlers/exit.spore
handler ExitHandler {
    exit(code: I32) -> Never {
        ffi_exit(code)
        // Never type: 函数永不返回
    }
}

// file: ffi.spore
foreign fn ffi_read_file(path: Bytes) -> FfiResult[Bytes]
foreign fn ffi_println(s: Bytes) -> Unit
foreign fn ffi_exit(code: I32) -> Never
```

---

## 4. spore.toml 中的 Platform 声明

### 4.1 单 Platform 配置

应用在 `spore.toml` 中声明使用的 Platform：

```toml
[package]
name = "my-app"
version = "0.1.0"

[platforms]
main = { git = "https://github.com/spore-platform/cli", version = "1.0.0" }
```

编译器会：
1. 下载 Platform 包（content-addressed）
2. 验证应用的 entry point 类型匹配 Platform 要求
3. 验证应用使用的所有 capability 都被 Platform 覆盖

### 4.2 多 Platform 配置

应用可以使用多个 Platform，通过 `priority` 字段决定 effect 路由：

```toml
[platforms]
cli = { git = "https://github.com/spore-platform/cli", priority = 1 }
gpu = { git = "https://github.com/spore-platform/gpu", priority = 2 }
```

**Priority 规则**：
- 数字越**小**，优先级越**高**
- 每个 effect 由优先级最高的、能够处理它的 Platform 负责
- 编译器确保每个 effect 有且仅有一个 handler

### 4.3 编译器验证

编译器在使用 Platform 时进行以下检查：

```spore
// app.spore
fn main(args: List[Str]) -> I32 ! [FileRead, GpuCompute, Exit] {
    let data = File.read("input.dat")  // FileRead
    let result = Gpu.matmul(data)      // GpuCompute
    0
}
```

```toml
[platforms]
cli = { git = ".../cli", priority = 1, handles = [FileRead, Exit] }
gpu = { git = ".../gpu", priority = 2, handles = [GpuCompute] }
```

编译器验证：
1. ✅ `FileRead` → `cli` Platform (priority 1)
2. ✅ `GpuCompute` → `gpu` Platform (priority 2)
3. ✅ `Exit` → `cli` Platform (priority 1)
4. ✅ 所有 effect 都有 handler
5. ✅ 没有 effect 冲突

如果配置错误：

```toml
# 错误：两个 Platform 都处理 FileRead
[platforms]
cli = { git = ".../cli", priority = 1, handles = [FileRead, Exit] }
backup = { git = ".../backup", priority = 2, handles = [FileRead] }
```

编译器报错：

```
error[E4201]: Effect handler conflict
  --> app.spore:2:17
   |
 2 |     let data = File.read("input.dat")
   |                 ^^^^^^^^^
   | FileRead can be handled by multiple platforms:
   |   - cli (priority 1)
   |   - backup (priority 2)
   | 
   | help: Remove one of the platforms or use explicit handler selection
```

### 4.4 条件 Platform

可以根据编译目标选择不同的 Platform：

```toml
[platforms]
default = { git = ".../cli" }

[target.wasm32-wasi.platforms]
default = { git = ".../wasm" }

[target.x86_64-unknown-linux.platforms]
default = { git = ".../linux-optimized" }
```

---

## 5. 多 Platform 支持

### 5.1 为什么需要多 Platform

某些应用需要结合多个领域的 capability：

- **CLI + GPU**：命令行工具需要进行 GPU 计算
- **Web + Database**：Web 服务需要数据库连接
- **CLI + Cloud**：命令行工具需要访问 S3/DynamoDB

Spore 允许应用同时使用多个 Platform，每个 Platform 负责不同的 capability 集合。

### 5.2 Capability 分区

多 Platform 的关键是 **capability 分区**：每个 Platform 处理一组不重叠的 capability。

```spore
// CLI Platform：文件和标准 IO
platform CliPlatform {
    handles [FileRead, FileWrite, StdOut, StdErr, Exit, Spawn]
    entry: fn(args: List[Str]) -> I32 ! [Exit]
}

// GPU Platform：GPU 计算
platform GpuPlatform {
    handles [GpuAlloc, GpuCompute, GpuTransfer]
    // 没有 entry point（不是主 Platform）
}
```

应用代码：

```spore
// app.spore
fn main(args: List[Str]) -> I32 ! [FileRead, GpuCompute, Exit] {
    // FileRead → CliPlatform
    let matrix = load_matrix_from_file("matrix.dat")
    
    // GpuCompute → GpuPlatform
    let result = Gpu.multiply(matrix, matrix)
    
    // StdOut → CliPlatform
    println(format("Result: {}", result))
    
    0
}
```

### 5.3 Priority 决定路由

当多个 Platform 都能处理同一个 capability 时，使用 `priority` 决定：

```toml
[platforms]
primary = { git = ".../primary", priority = 1, handles = [FileRead, NetRead] }
fallback = { git = ".../fallback", priority = 2, handles = [FileRead, NetRead] }
```

- `FileRead` effect → `primary` Platform（优先级 1）
- 如果 `primary` 不可用，不会自动 fallback（需要显式错误处理）

### 5.4 完整示例：ML 训练应用

```toml
# spore.toml
[package]
name = "ml-trainer"
version = "0.1.0"

[platforms]
cli = { git = "https://github.com/spore-platform/cli", priority = 1 }
gpu = { git = "https://github.com/spore-platform/cuda", priority = 2 }
s3 = { git = "https://github.com/spore-platform/aws", priority = 3 }
```

```spore
// app.spore
fn main(args: List[Str]) -> I32 ! [FileRead, GpuCompute, S3Write, Exit] {
    // 1. 从本地读取配置（CLI Platform）
    let config = File.read("config.toml") |> parse_config
    
    // 2. 从 S3 加载训练数据（S3 Platform）
    let data = S3.get("ml-datasets", "train.parquet")
    
    // 3. 在 GPU 上训练模型（GPU Platform）
    let model = train_model_on_gpu(data, config)
    
    // 4. 保存模型到 S3（S3 Platform）
    S3.put("ml-models", "model-v1.bin", model)
    
    println("Training complete!")
    0
}

fn train_model_on_gpu(data: Tensor, config: Config) -> Model ! [GpuCompute] {
    let gpu_mem = Gpu.alloc(data.size())
    Gpu.transfer_to(gpu_mem, data)
    
    for epoch in 0..config.epochs {
        Gpu.matmul(gpu_mem, config.weights)
        // ...
    }
    
    let result = Gpu.transfer_from(gpu_mem)
    Gpu.free(gpu_mem)
    result
}
```

编译器生成的 effect 路由表：

| Effect | Handler Platform | Priority |
|--------|------------------|----------|
| FileRead | cli | 1 |
| Exit | cli | 1 |
| GpuCompute | gpu | 2 |
| GpuAlloc | gpu | 2 |
| GpuTransfer | gpu | 2 |
| S3Read | s3 | 3 |
| S3Write | s3 | 3 |

### 5.5 编译器错误：Capability 缺失

如果应用使用了未被任何 Platform 覆盖的 capability：

```spore
fn main(args: List[Str]) -> I32 ! [FileRead, DatabaseQuery, Exit] {
    let rows = Db.query("SELECT * FROM users")  // DatabaseQuery
    // ...
}
```

```toml
[platforms]
cli = { git = ".../cli" }  # 只提供 FileRead, Exit
```

编译器报错：

```
error[E4202]: Missing effect handler
  --> app.spore:2:16
   |
 2 |     let rows = Db.query("SELECT * FROM users")
   |                ^^^^^^^^^
   | Effect 'DatabaseQuery' is not handled by any platform
   | 
   | help: Add a platform that provides DatabaseQuery:
   |   [platforms]
   |   db = { git = "https://github.com/spore-platform/postgres" }
```

---

## 6. 应用代码与 Platform 的交互

### 6.1 应用代码视角

从应用代码的角度，**Platform 是完全透明的**。应用只需要：

1. 声明需要的 capability
2. 正常调用 capability 提供的函数
3. Platform handler 自动处理

```spore
// 应用代码
fn read_and_process(path: Path) -> Result[Data, Error] ! [FileRead] {
    // 看起来像普通函数调用，实际上发出 effect
    let content = File.read(path)?
    parse_data(content)
}
```

编译器转换为：

```spore
// 编译器内部表示（伪代码）
fn read_and_process(path: Path) -> Result[Data, Error] ! [FileRead] {
    // 生成 effect，由 Platform handler 处理
    perform FileRead.read(path) with handler -> {
        let content = handler.result?
        parse_data(content)
    }
}
```

### 6.2 Effect 在类型系统中的表示

Spore 的类型系统追踪所有 effect：

```spore
// 纯函数：没有 effect
fn add(x: I32, y: I32) -> I32 {
    x + y
}

// 有 effect 的函数：使用 ! [Effects]
fn read_number(path: Path) -> I32 ! [FileRead] {
    let s = File.read_to_string(path)
    s.parse_i32().unwrap()
}

// 多个 effect
fn fetch_and_save(url: Url, dest: Path) -> Unit ! [NetRead, FileWrite] {
    let data = Http.get(url)
    File.write(dest, data)
}
```

Effect 传播：

```spore
fn caller() -> Unit ! [FileRead] {
    // 调用有 effect 的函数，effect 会传播
    let n = read_number("num.txt")
    println("Number: {}", n)  // println 没有 effect（纯函数）
}
```

### 6.3 完整应用示例：配置文件读取器

```spore
// app.spore
module App

uses [FileRead, StdOut, Exit]

fn main(args: List[Str]) -> I32 ! [Exit] {
    match read_config() {
        Ok(config) -> {
            println("Loaded config: {}", config)
            0
        }
        Err(err) -> {
            eprintln("Error: {}", err)
            1
        }
    }
}

fn read_config() -> Result[Config, Error] ! [FileRead] {
    let content = File.read_to_string("config.toml")?
    parse_toml(content)
}

type Config {
    host: Str,
    port: U16,
    debug: Bool,
}

// 纯函数：解析 TOML（不需要 IO）
fn parse_toml(s: Str) -> Result[Config, Error] {
    // ...
}
```

```toml
# spore.toml
[package]
name = "config-reader"
version = "0.1.0"

[platforms]
main = { git = "https://github.com/spore-platform/cli" }
```

编译并运行：

```bash
$ spore build
Compiling config-reader v0.1.0
  - Resolving platform: cli v1.0.0
  - Verifying effect handlers: ✓ FileRead, ✓ StdOut, ✓ Exit
  - Generating executable
    Finished release [optimized] target(s) in 2.34s

$ ./config-reader
Loaded config: Config { host: "localhost", port: 8080, debug: true }
```

### 6.4 Effect 组合

应用可以定义自己的高级 effect，然后映射到 Platform effect：

```spore
// 应用定义的 effect
effect Logger {
    fn log_info(msg: Str) -> Unit
    fn log_error(msg: Str) -> Unit
}

// 将 Logger 映射到 Platform 的 StdOut/StdErr
handler ConsoleLogger uses [StdOut, StdErr] {
    log_info(msg: Str) {
        println("[INFO] {}", msg)
        resume(())
    }
    
    log_error(msg: Str) {
        eprintln("[ERROR] {}", msg)
        resume(())
    }
}

// 应用代码使用高级 effect
fn process_data(data: Data) -> Unit ! [Logger] {
    log_info("Processing data...")
    // ...
    log_info("Done!")
}

// 在 main 中安装 handler
fn main(args: List[Str]) -> I32 ! [Exit, StdOut, StdErr] {
    with ConsoleLogger {
        process_data(load_data())
    }
    0
}
```

### 6.5 完整示例：HTTP 客户端

```spore
// app.spore
module HttpClient

uses [NetRead, NetWrite, StdOut]

fn main(args: List[Str]) -> I32 ! [Exit] {
    let url = "https://api.github.com/users/octocat"
    
    match fetch_user(url) {
        Ok(user) -> {
            println("User: {}", user.name)
            println("Repos: {}", user.public_repos)
            0
        }
        Err(err) -> {
            eprintln("Failed to fetch: {}", err)
            1
        }
    }
}

fn fetch_user(url: Str) -> Result[GithubUser, HttpError] ! [NetRead, NetWrite] {
    let response = Http.get(url)?
    
    if response.status != 200 {
        return Err(HttpError.BadStatus(response.status))
    }
    
    let user = Json.parse(response.body)?
    Ok(user)
}

type GithubUser {
    name: Str,
    public_repos: I32,
}
```

---

## 7. 测试 Platform

### 7.1 为什么需要 Test Platform

由于应用代码与 Platform 解耦，我们可以为测试提供一个**确定性的 Mock Platform**：

- **不需要真实文件系统**：文件操作返回预定义数据
- **不需要网络**：HTTP 请求返回 mock 响应
- **可重现**：每次运行测试结果完全一致
- **快速**：没有 IO 延迟

### 7.2 Test Platform 模式

```spore
// test_platform.spore
platform TestPlatform {
    handles [FileRead, FileWrite, NetRead, NetWrite, Clock]
    
    entry: fn() -> TestResult
}

// Mock 文件系统
handler MockFileSystem {
    // 内存中的虚拟文件系统
    var fs: Map[Path, Bytes] = Map.new()
    
    File.read(path: Path) -> Result[Bytes, IoError] {
        match fs.get(path) {
            Some(content) -> resume(Ok(content))
            None -> resume(Err(IoError.NotFound))
        }
    }
    
    File.write(path: Path, content: Bytes) -> Result[Unit, IoError] {
        fs.set(path, content)
        resume(Ok(()))
    }
}

// Mock 网络
handler MockNetwork {
    var responses: Map[Url, Response] = Map.new()
    
    Http.get(url: Url) -> Result[Response, HttpError] {
        match responses.get(url) {
            Some(resp) -> resume(Ok(resp))
            None -> resume(Err(HttpError.NotFound))
        }
    }
}

// 确定性时钟
handler MockClock {
    var current_time: U64 = 0
    
    Clock.now() -> Timestamp {
        resume(Timestamp(current_time))
    }
    
    // Test helper: 推进时间
    fn advance(millis: U64) {
        current_time += millis
    }
}
```

### 7.3 测试应用代码

```spore
// app.spore
fn read_and_parse(path: Path) -> Result[Config, Error] ! [FileRead] {
    let content = File.read_to_string(path)?
    parse_config(content)
}

// test/app_test.spore
test "read_and_parse with valid config" {
    // 设置 mock 文件系统
    TestPlatform.mock_file("config.toml", """
        host = "localhost"
        port = 8080
    """)
    
    // 运行测试
    let result = read_and_parse("config.toml")
    
    // 断言
    assert result.is_ok()
    assert result.unwrap().host == "localhost"
}

test "read_and_parse with missing file" {
    // 不设置文件 → File.read 返回 NotFound
    let result = read_and_parse("missing.toml")
    
    assert result.is_err()
    assert result.unwrap_err() == Error.FileNotFound
}
```

### 7.4 Record/Replay 模式

对于集成测试，可以先在真实 Platform 上运行一次，记录所有 IO 操作，然后在测试中重放：

```spore
// 1. 在真实 Platform 上运行，记录 IO
platform RecordingPlatform {
    handles [FileRead, FileWrite, NetRead, NetWrite]
    
    handler RecordingHandler {
        var log: List[IoEvent] = []
        
        File.read(path: Path) -> Result[Bytes, IoError] {
            let result = real_file_read(path)
            log.push(IoEvent.FileRead(path, result))
            resume(result)
        }
        
        Http.get(url: Url) -> Result[Response, HttpError] {
            let result = real_http_get(url)
            log.push(IoEvent.HttpGet(url, result))
            resume(result)
        }
        
        fn save_log(path: Path) {
            File.write(path, serialize(log))
        }
    }
}

// 2. 在测试中重放
platform ReplayPlatform {
    handles [FileRead, NetRead]
    
    handler ReplayHandler {
        var log: List[IoEvent] = load_log("test_recording.log")
        var index: U32 = 0
        
        File.read(path: Path) -> Result[Bytes, IoError] {
            let event = log[index]
            index += 1
            
            match event {
                IoEvent.FileRead(p, result) if p == path -> resume(result)
                _ -> panic("Unexpected IO: expected FileRead({})", path)
            }
        }
    }
}
```

### 7.5 完整测试示例

```spore
// app.spore
fn fetch_weather(city: Str) -> Result[Weather, Error] ! [NetRead] {
    let url = "https://api.weather.com/v1/current?city={}"
    let response = Http.get(format(url, city))?
    Json.parse(response.body)
}

// test/weather_test.spore
module WeatherTest

test "fetch_weather returns valid data" {
    // 设置 mock HTTP 响应
    TestPlatform.mock_http(
        url = "https://api.weather.com/v1/current?city=Beijing",
        response = Response {
            status = 200,
            body = """{"temp": 15, "condition": "Sunny"}""",
        }
    )
    
    // 运行测试
    let weather = fetch_weather("Beijing").unwrap()
    
    // 断言
    assert weather.temp == 15
    assert weather.condition == "Sunny"
}

test "fetch_weather handles network error" {
    // Mock 网络错误
    TestPlatform.mock_http_error(
        url = "https://api.weather.com/v1/current?city=Invalid",
        error = HttpError.ConnectionFailed
    )
    
    let result = fetch_weather("Invalid")
    assert result.is_err()
}

test "fetch_weather handles malformed JSON" {
    TestPlatform.mock_http(
        url = "https://api.weather.com/v1/current?city=London",
        response = Response {
            status = 200,
            body = "not json",
        }
    )
    
    let result = fetch_weather("London")
    assert result.is_err()
    match result.unwrap_err() {
        Error.JsonParse(_) -> {}  // 正确
        _ -> panic("Expected JsonParse error")
    }
}
```

### 7.6 `spore test` 自动使用 Test Platform

运行 `spore test` 时，编译器自动使用 Test Platform：

```bash
$ spore test
   Compiling weather-app v0.1.0
   Using test platform: spore-platform/test v1.0.0
   Running 3 tests

test fetch_weather_returns_valid_data ... ok (0.001s)
test fetch_weather_handles_network_error ... ok (0.001s)
test fetch_weather_handles_malformed_json ... ok (0.001s)

Test result: ok. 3 passed; 0 failed; 0 ignored
```

可以在 `spore.toml` 中配置测试 Platform：

```toml
[platforms]
main = { git = ".../cli" }

[test.platforms]
test = { git = ".../test-platform" }
```

---

## 8. Platform 开发指南

### 8.1 创建新 Platform 的步骤

假设我们要创建一个 `EmbeddedPlatform`，用于嵌入式设备：

**步骤 1**：初始化 Platform 项目

```bash
$ spore new platform embedded-platform
Created platform package: embedded-platform/
```

**步骤 2**：定义 Platform 契约

```spore
// platform.spore
platform EmbeddedPlatform {
    version: "0.1.0"
    
    // 嵌入式设备的 capability
    handles [
        GpioRead,      // 读 GPIO
        GpioWrite,     // 写 GPIO
        Timer,         // 定时器
        SerialRead,    // 串口读
        SerialWrite,   // 串口写
    ]
    
    // Entry point：设备初始化 + 主循环
    entry: fn() -> Never ! [GpioRead, GpioWrite, Timer]
    
    config: {
        cpu_freq: U32,        // CPU 频率
        gpio_pins: U8,        // GPIO 引脚数量
    }
}
```

**步骤 3**：实现 Effect Handler

```spore
// handlers/gpio.spore
handler GpioHandler {
    Gpio.read(pin: U8) -> Bool {
        // FFI 调用底层硬件
        resume(ffi_gpio_read(pin))
    }
    
    Gpio.write(pin: U8, value: Bool) -> Unit {
        ffi_gpio_write(pin, value)
        resume(())
    }
    
    Gpio.set_mode(pin: U8, mode: GpioMode) -> Unit {
        ffi_gpio_set_mode(pin, mode as U8)
        resume(())
    }
}

// handlers/timer.spore
handler TimerHandler {
    Timer.delay_ms(ms: U32) -> Unit {
        ffi_delay_ms(ms)
        resume(())
    }
    
    Timer.millis() -> U32 {
        resume(ffi_timer_millis())
    }
}
```

**步骤 4**：FFI 实现（Rust）

```rust
// ffi/src/lib.rs
use core::ptr;

// GPIO 寄存器地址（取决于具体硬件）
const GPIO_BASE: usize = 0x4000_0000;

#[no_mangle]
pub extern "C" fn ffi_gpio_read(pin: u8) -> bool {
    unsafe {
        let reg = (GPIO_BASE + pin as usize * 4) as *const u32;
        ptr::read_volatile(reg) != 0
    }
}

#[no_mangle]
pub extern "C" fn ffi_gpio_write(pin: u8, value: bool) {
    unsafe {
        let reg = (GPIO_BASE + pin as usize * 4) as *mut u32;
        ptr::write_volatile(reg, if value { 1 } else { 0 });
    }
}

// 定时器实现（基于 systick）
static mut TICK_COUNT: u32 = 0;

#[no_mangle]
pub extern "C" fn ffi_timer_millis() -> u32 {
    unsafe { TICK_COUNT }
}

#[no_mangle]
pub extern "C" fn ffi_delay_ms(ms: u32) {
    let start = ffi_timer_millis();
    while ffi_timer_millis() - start < ms {
        // 忙等待
    }
}
```

**步骤 5**：编写文档和示例

```markdown
# Embedded Platform

用于嵌入式 MCU 的 Spore Platform。

## 支持的硬件

- STM32F4 系列
- ESP32

## 使用方法

\`\`\`toml
[platforms]
main = { git = "https://github.com/you/embedded-platform" }
\`\`\`

\`\`\`spore
fn main() -> Never ! [GpioWrite, Timer] {
    // LED 引脚
    Gpio.set_mode(13, GpioMode.Output)
    
    loop {
        Gpio.write(13, true)   // LED 亮
        Timer.delay_ms(500)
        Gpio.write(13, false)  // LED 灭
        Timer.delay_ms(500)
    }
}
\`\`\`
```

**步骤 6**：发布 Platform

```bash
$ spore publish
Publishing embedded-platform v0.1.0
  - Computing content hash: sha256:a3b2c1...
  - Uploading to distributed storage
  - Registering platform metadata
Published: platform:embedded-platform@sha256:a3b2c1...
```

### 8.2 Handler 实现模式

**模式 1**：直接 FFI

```spore
handler DirectFfi {
    SomeOp.call(arg: T) -> R {
        resume(ffi_some_op(arg))
    }
}
```

**模式 2**：组合其他 Effect

```spore
handler ComposedHandler uses [LowerLevel] {
    HighLevel.call(arg: T) -> R {
        let x = lower_level_op1(arg)
        let y = lower_level_op2(x)
        resume(y)
    }
}
```

**模式 3**：有状态的 Handler

```spore
handler StatefulHandler {
    var state: Map[K, V] = Map.new()
    
    Cache.get(key: K) -> Option[V] {
        resume(state.get(key))
    }
    
    Cache.set(key: K, value: V) {
        state.insert(key, value)
        resume(())
    }
}
```

### 8.3 Platform 测试

Platform 本身也需要测试：

```spore
// test/gpio_test.spore
test "gpio write and read" {
    // 使用真实硬件或模拟器
    Gpio.set_mode(5, GpioMode.Output)
    Gpio.write(5, true)
    
    // 回读（如果硬件支持）
    let value = Gpio.read(5)
    assert value == true
}
```

对于需要硬件的 Platform，使用硬件模拟器：

```bash
$ spore test --platform-config '{"use_simulator": true}'
```

---

## 9. 与其他子系统的交互

### 9.1 与并发模型的交互

Spore 的并发模型也是基于 effect handler，`Spawn` 就是一个 effect：

```spore
// 并发 effect
effect Concurrency {
    fn spawn<T>(f: fn() -> T) -> Task[T]
    fn await<T>(task: Task[T]) -> T
}

// Platform 提供 Spawn handler
handler SpawnHandler {
    spawn<T>(f: fn() -> T) -> Task[T] {
        // 使用 Platform 的运行时（如 tokio, async-std）
        let task_id = runtime_spawn(f)
        resume(Task(task_id))
    }
    
    await<T>(task: Task[T]) -> T {
        let result = runtime_await(task.id)
        resume(result)
    }
}
```

应用代码：

```spore
fn parallel_fetch(urls: List[Url]) -> List[Response] ! [NetRead, Spawn] {
    let tasks = urls.map(|url| Task.spawn(|| Http.get(url)))
    tasks.map(Task.await)
}
```

### 9.2 与包管理系统的交互

Platform 是一个普通的 Spore 包，遵循相同的包管理规则：

- **Content-addressed**：Platform 通过内容哈希唯一标识
- **Dual hash**：sig hash + impl hash
- **分布式获取**：从 Git/IPFS/HTTP 获取

```toml
[platforms]
cli = {
    git = "https://github.com/spore-platform/cli",
    version = "1.0.0",
    sig_hash = "sha256:a1b2c3...",   # 签名哈希
    impl_hash = "sha256:d4e5f6...",  # 实现哈希
}
```

编译器缓存 Platform：

```
~/.spore/platforms/
  cli-sha256-a1b2c3.../
    platform.spore
    handlers/
    ffi/
```

### 9.3 与 Capability 系统的交互

Platform **定义了 capability 的上界**：

```spore
// 应用声明需要的 capability
module App
uses [FileRead, NetWrite]

fn main() { ... }
```

编译器检查：
1. 应用使用的每个 capability 都必须被某个 Platform 处理
2. Platform 提供的 capability 是应用所需 capability 的超集

```
App.uses = {FileRead, NetWrite}
Platform.handles = {FileRead, FileWrite, NetRead, NetWrite, Clock}

Check: App.uses ⊆ Platform.handles  ✓
```

如果应用尝试使用 Platform 不支持的 capability：

```spore
module App
uses [DatabaseQuery]  // Platform 不支持

fn main() {
    Db.query("SELECT ...")  // 编译错误
}
```

```
error[E4202]: Missing effect handler
  | Platform 'cli' does not provide capability 'DatabaseQuery'
```

### 9.4 与成本模型的交互

Platform effect 的成本标记：

```spore
platform CliPlatform {
    handles [FileRead @cost(io=1, mem=0), NetRead @cost(io=10, mem=0)]
}

handler FileReadHandler {
    File.read(path: Path) -> Result[Bytes, IoError]
        @cost(io=call, mem=result.size()) 
    {
        resume(ffi_read_file(path))
    }
}
```

编译器生成成本分析：

```spore
fn load_data() -> Data ! [FileRead] {
    let f1 = File.read("a.txt")  // cost: io=1, mem=?
    let f2 = File.read("b.txt")  // cost: io=1, mem=?
    combine(f1, f2)
}

// Total cost: io=2, mem=<depends on file sizes>
```

### 9.5 编译器输出中的 Platform 信息

编译器在各个阶段输出 Platform 相关信息：

**解析阶段**：

```
Resolving platforms:
  - cli: https://github.com/spore-platform/cli @ v1.0.0
  - Downloading: 100% [====================] 2.3 MB
  - Verifying signature: sha256:a1b2c3... ✓
```

**类型检查阶段**：

```
Checking effect coverage:
  - FileRead: ✓ handled by 'cli'
  - NetWrite: ✓ handled by 'cli'
  - GpuCompute: ✓ handled by 'gpu'
  - All effects covered ✓
```

**代码生成阶段**：

```
Generating platform bindings:
  - cli: linking ffi_read_file, ffi_write_file, ...
  - gpu: linking cuda_alloc, cuda_matmul, ...
```

**错误代码**：

- `E4201`: Effect handler conflict（多个 Platform 处理同一 effect）
- `E4202`: Missing effect handler（没有 Platform 处理某个 effect）
- `E4203`: Entry point type mismatch（应用入口类型与 Platform 不符）
- `E4204`: Platform download failed
- `E4205`: Platform signature verification failed

---

## 10. 设计决策记录 (ADR)

### ADR-001: Platform 是语言级概念

**决策**：Platform 在 `spore.toml` 中声明，编译器负责验证和链接。

**理由**：
- 编译时检查：确保所有 effect 都有 handler
- 类型安全：验证 entry point 类型匹配
- 优化：编译器可以内联 effect handler

**替代方案**：Platform 作为库（运行时加载）
- ❌ 无法在编译时验证 effect 覆盖
- ❌ 运行时错误风险高

---

### ADR-002: 所有 IO 通过 Platform

**决策**：应用代码不包含任何 IO 实现，全部由 Platform 提供。

**理由**：
- **可测试性**：应用代码完全纯函数，可以用 mock Platform 测试
- **可移植性**：相同应用代码可以在不同 Platform 上运行（CLI/Web/Embedded）
- **安全性**：IO 权限由 Platform 控制，应用无法绕过

**替代方案**：标准库提供 IO
- ❌ 无法替换 IO 实现
- ❌ 测试需要 mock 整个标准库

---

### ADR-003: Effect Handler 风格

**决策**：使用 algebraic effect handler，与并发模型统一。

**理由**：
- **一致性**：`Spawn` 也是 effect，统一处理
- **组合性**：可以组合多个 handler
- **表达力**：handler 可以拦截、转换、重试 effect

**替代方案**：Monad transformer（如 Haskell）
- ❌ 组合性差
- ❌ 类型复杂

---

### ADR-004: 没有内置 Platform

**决策**：Spore 不提供内置 Platform，全部第三方。

**理由**：
- **灵活性**：用户可以选择最适合的 Platform
- **演进**：Platform 可以独立于语言版本更新
- **社区驱动**：鼓励社区创建专用 Platform

参考：Roc 语言的设计

**替代方案**：内置标准 Platform
- ❌ 增加语言复杂性
- ❌ 限制创新

---

### ADR-005: Git URL 指定 Platform

**决策**：在 `spore.toml` 中使用 `git = "url"` 指定 Platform。

**理由**：
- **去中心化**：不依赖中央 registry
- **版本控制**：Git 提供完整的版本历史
- **内容寻址**：结合 content hash 确保不可篡改

**替代方案**：中央 registry（如 crates.io）
- ❌ 单点故障
- ❌ 审查风险

---

### ADR-006: 支持多 Platform

**决策**：允许应用使用多个 Platform，通过 priority 决定路由。

**理由**：
- **组合性**：不同领域的 capability 可以由专用 Platform 提供
- **现实需求**：GPU 计算 + 文件 IO + 数据库访问

**约束**：
- 每个 effect 必须有且仅有一个 handler
- 编译器验证无歧义

---

### ADR-007: Platform 契约

**决策**：Platform 通过三部分定义契约：capability 集合、handler 实现、entry point 类型。

**理由**：
- **明确性**：应用知道 Platform 提供什么
- **可验证**：编译器可以检查契约满足
- **文档**：契约即文档

---

### ADR-008: Native 实现语言

**决策**：Platform 的底层实现使用 native 代码（Rust/C/编译后的 Spore）。

**理由**：
- **性能**：IO 操作需要高效实现
- **FFI**：直接调用操作系统 API
- **生态**：利用现有 Rust/C 生态

**替代方案**：纯 Spore 实现
- ❌ 无法调用系统 API
- ❌ 性能不足

---

### ADR-009: 测试 Platform 模式

**决策**：提供 Test Platform，用于单元测试，返回确定性结果。

**理由**：
- **可测试性**：测试不依赖真实文件系统/网络
- **速度**：内存操作比真实 IO 快 1000 倍
- **可重现**：相同测试每次结果一致

参考：Elm 的 test runner 设计

---

## 11. 附录

### 11.1 Platform API 完整示例

#### CLI Platform API

```spore
platform CliPlatform {
    version: "1.0.0"
    handles [FileRead, FileWrite, StdOut, StdErr, NetRead, NetWrite, Clock, Spawn, Exit]
    entry: fn(args: List[Str]) -> I32 ! [Exit]
}

// File API
effect FileRead {
    fn read(path: Path) -> Result[Bytes, IoError]
    fn read_to_string(path: Path) -> Result[Str, IoError]
    fn exists(path: Path) -> Bool
    fn list_dir(path: Path) -> Result[List[DirEntry], IoError]
    fn metadata(path: Path) -> Result[FileMetadata, IoError]
}

effect FileWrite {
    fn write(path: Path, content: Bytes) -> Result[Unit, IoError]
    fn append(path: Path, content: Bytes) -> Result[Unit, IoError]
    fn delete(path: Path) -> Result[Unit, IoError]
    fn create_dir(path: Path) -> Result[Unit, IoError]
}

// Network API
effect NetRead {
    fn tcp_connect(host: Str, port: U16) -> Result[TcpSocket, NetError]
}

effect NetWrite {
    fn tcp_send(socket: TcpSocket, data: Bytes) -> Result[U64, NetError]
}

// Standard IO
effect StdOut {
    fn println(s: Str) -> Unit
    fn print(s: Str) -> Unit
}

effect StdErr {
    fn eprintln(s: Str) -> Unit
}

// Clock
effect Clock {
    fn now() -> Timestamp
    fn sleep(duration: Duration) -> Unit
}

// Concurrency
effect Spawn {
    fn spawn<T>(f: fn() -> T) -> Task[T]
    fn await<T>(task: Task[T]) -> T
    fn spawn_blocking<T>(f: fn() -> T) -> Task[T]
}

// Exit
effect Exit {
    fn exit(code: I32) -> Never
}
```

#### Web Platform API

```spore
platform WebPlatform {
    version: "1.0.0"
    handles [HttpServer, HttpClient, DbQuery, Clock, Spawn]
    entry: fn(req: Request) -> Response ! [HttpServer, DbQuery]
}

effect HttpServer {
    fn listen(port: U16, handler: fn(Request) -> Response) -> Never
    fn parse_request(raw: Bytes) -> Request
    fn render_response(resp: Response) -> Bytes
}

effect HttpClient {
    fn get(url: Url) -> Result[Response, HttpError]
    fn post(url: Url, body: Bytes) -> Result[Response, HttpError]
}

effect DbQuery {
    fn query<T>(sql: Str) -> Result[List[T], DbError]
    fn execute(sql: Str) -> Result[U64, DbError]
    fn transaction<T>(f: fn() -> T) -> Result[T, DbError]
}

type Request {
    method: HttpMethod,
    path: Str,
    headers: Map[Str, Str],
    body: Bytes,
}

type Response {
    status: U16,
    headers: Map[Str, Str],
    body: Bytes,
}
```

### 11.2 与 Roc 的对比

| 特性 | Roc | Spore |
|------|-----|-------|
| Platform 概念 | ✓ | ✓ |
| Effect System | ✗（使用 tag unions） | ✓（algebraic effects） |
| 多 Platform | ✗ | ✓ |
| Entry Point | 固定签名 | Platform 定义 |
| Concurrency | Platform 提供 | Effect handler |
| 测试 | Mock Platform | Test Platform + deterministic handlers |

Spore 的优势：
- **Effect System**：更好的组合性和类型推导
- **多 Platform**：可以组合多个 Platform
- **统一并发**：Spawn 也是 effect

Roc 的优势：
- **简单性**：没有 effect system 的学习曲线
- **成熟度**：Roc Platform 生态更完善

### 11.3 与 Elm 的对比

Elm 的 `Cmd` 和 `Sub` 类似于 effect，但有重要区别：

| 特性 | Elm | Spore Platform |
|------|-----|----------------|
| IO 模型 | `Cmd Msg`（命令） | Effect handler |
| 可测试性 | 中等（需要 mock Cmd） | 极高（Test Platform） |
| 类型系统 | `Task` Monad | Effect types |
| 并发 | 内置 runtime | Platform 提供 |
| 可扩展性 | 固定（只有 Cmd/Sub） | 完全可扩展 |

Spore 更灵活，Elm 更简单。

### 11.4 与 Koka 的对比

Koka 也使用 algebraic effect handler，但没有 Platform 概念：

| 特性 | Koka | Spore |
|------|------|-------|
| Effect Handler | ✓ | ✓ |
| Platform | ✗ | ✓ |
| IO 模型 | 内置 IO handler | Platform 提供 |
| 测试 | 需要手动 mock | Test Platform |

Spore = Koka 的 effect system + Roc 的 Platform 概念

### 11.5 完整应用示例：Web 服务

```spore
// app.spore
module TodoApi

uses [HttpServer, DbQuery, Clock, Spawn]

fn main(req: Request) -> Response ! [HttpServer, DbQuery] {
    match (req.method, req.path) {
        (GET, "/todos") -> list_todos(req)
        (POST, "/todos") -> create_todo(req)
        (GET, "/todos/:id") -> get_todo(req)
        (PUT, "/todos/:id") -> update_todo(req)
        (DELETE, "/todos/:id") -> delete_todo(req)
        _ -> Response.not_found()
    }
}

fn list_todos(req: Request) -> Response ! [DbQuery] {
    let todos = Db.query("SELECT * FROM todos ORDER BY created_at DESC")
    Response.json(todos)
}

fn create_todo(req: Request) -> Response ! [DbQuery, Clock] {
    let todo: TodoInput = Json.parse(req.body)?
    
    let now = Clock.now()
    let id = Db.execute("""
        INSERT INTO todos (title, completed, created_at)
        VALUES (?, ?, ?)
        RETURNING id
    """, [todo.title, false, now])?
    
    Response.json({ id, title: todo.title, completed: false })
}

type TodoInput {
    title: Str,
}

type Todo {
    id: I64,
    title: Str,
    completed: Bool,
    created_at: Timestamp,
}
```

```toml
# spore.toml
[package]
name = "todo-api"
version = "0.1.0"

[platforms]
web = { git = "https://github.com/spore-platform/web", version = "1.0.0" }
```

运行：

```bash
$ spore build
$ ./todo-api
Server listening on http://localhost:8080
```

测试：

```spore
// test/api_test.spore
test "create todo" {
    TestPlatform.mock_db_execute(
        sql = "INSERT INTO todos ...",
        result = Ok(1)  // 返回 ID = 1
    )
    
    TestPlatform.mock_clock(timestamp = 1672531200)
    
    let req = Request {
        method = POST,
        path = "/todos",
        body = """{"title": "Buy milk"}""",
    }
    
    let resp = main(req)
    assert resp.status == 200
    
    let todo: Todo = Json.parse(resp.body)
    assert todo.id == 1
    assert todo.title == "Buy milk"
}
```

---

## 12. 总结

Spore 的 Platform 系统实现了以下目标：

1. **纯函数应用**：应用代码完全纯函数，不包含 IO 实现
2. **可测试性**：通过 Test Platform 实现确定性测试
3. **可移植性**：相同代码可以在不同 Platform 上运行
4. **组合性**：支持多个 Platform 组合
5. **安全性**：IO 权限由 Platform 控制
6. **可扩展性**：社区可以创建专用 Platform

Platform 是 Spore 设计的核心，使得 Spore 成为真正的 **pure functional language with practical IO**。

---

**下一步**：

- [ ] 实现 CLI Platform 原型
- [ ] 实现 Test Platform
- [ ] 编写 Platform 开发工具
- [ ] 建立 Platform 生态

---

**变更历史**：

- 2025-01-03: v0.1 初始版本
