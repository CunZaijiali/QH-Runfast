# RunFast (`runf`)

> Start configured programs quickly —— 按名字或别名快速启动本机程序的小工具。

把「一长串绝对路径」变成「一个短名字」。配置文件是纯 TOML，改起来不用重新编译。

```
$ runf code        # 启动 VS Code
$ runf obs         # 启动 OBS Studio
$ runf list        # 看看都配了些什么
$ runf search sql  # 按关键词找
```

## 特性

- **按名字 / 别名启动**：大小写不敏感，自动去掉首尾空格
- **多路径回退**：一条目可写多个候选路径，按顺序尝试，第一个成功即用
- **关键词搜索**：`search` 会在 name / alias / keywords / path / description 里找
- **工作目录自动处理**：未显式指定时，自动把子进程的工作目录设为程序自身所在目录
  （等价于 Windows 快捷方式的「起始位置」，可解决 OBS 这类程序找不到自身 `locale`/`data` 的问题）
- **`.bat` / `.cmd` 支持**：自动经 `cmd /S /C` 启动
- **批量启动**：一条命令可同时启动多个目标
- **进程跟踪**：`--track` 等待子进程结束并汇报退出状态
- **配置自检**：重复的 `name` / `alias` 会在启动前被拒绝，避免「配错了却不知道」

## 构建

需要 Rust 工具链（edition 2024）。

```bash
git clone <this-repo>
cd run-fast
cargo build --release
```

产物在 `target/release/run-fast.exe`。把它改名（或复制）为 `runf.exe`，
放进任意已在 `PATH` 上的目录即可，例如：

```powershell
Copy-Item .\target\release\run-fast.exe "$env:LOCALAPPDATA\Programs\runf\runf.exe"
```

> 低于 `0.1.0` 的版本号、以及 `runf` 这个命令名，都由源码里的常量决定。

## 快速开始

1. 建一份配置（见下节），例如 `D:\GlobalScripts\bin\config.toml`
2. 把 `runf.exe` 放到 PATH 上
3. 运行：

```
runf list
```

## 配置

配置文件是 TOML，顶层是多个 `[[item]]`：

```toml
[[item]]
name = "Visual Studio Code"          # 必填，且全局唯一
alias = ["code", "vsc"]              # 可选，可多个；与 name 共用同一「标识符」命名空间
keywords = ["ide", "editor", "vscode"]  # 可选，仅用于 search
path = [                             # 可选，可多个；按顺序尝试
    "D:/Microsoft VS Code/Code.exe",
]
start_args = []                      # 可选，传给程序的启动参数
work_dir = ""                        # 可选，留空 = 程序自身所在目录
description = "代码编辑器"            # 可选，仅用于 list / search 展示
```

### 字段说明

| 字段 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `name` | string | ✅ | 主名字，也是默认的启动标识；不能为空 |
| `alias` | string[] | | 额外标识（短名、缩写、拼音等） |
| `keywords` | string[] | | 仅影响 `search`，不影响启动匹配 |
| `path` | string[] | | 候选程序路径，按顺序回退；相对路径按**配置文件所在目录**解析 |
| `start_args` | string[] | | 启动参数 |
| `work_dir` | string | | 子进程工作目录；**留空则自动用程序自身所在目录** |
| `description` | string | | 描述文本，仅展示用 |

> `[meta]` 段目前**不被程序读取**（解析器会忽略未知字段），可以保留做注释用途。

### 命名冲突规则（重要）

`name` 与所有 `alias` **共享同一个命名空间**，比较时会统一转小写。因此：

- 两个条目不能有相同的 `name` 或 `alias`
- **同一个条目内，`alias` 也不能和它自己的 `name` 相同**

一旦出现重复，程序会在加载配置时报错并拒绝运行（连 `list` 都跑不了）：

```
error: duplicate name or alias in configuration: 'cheat engine'
```

常见踩法：单字名字（`Everything`、`Spotify`、`Steam`）又配了一个同字面的别名。

### 配置查找顺序

依次尝试，第一个存在的即被采用：

1. `--config <FILE>` 指定的文件
2. 环境变量 `RUNFAST_CONFIG` 指向的文件
3. 可执行文件所在目录下的 `config.toml`
4. 可执行文件父目录下的 `config.toml`
5. 当前工作目录下的 `config.toml`

## 命令行用法

```
Usage: runf [OPTIONS] [TARGET]... [COMMAND]

Commands:
  search, -s, --search  Search configured programs
  list,   -l, --list    List all configured programs

Arguments:
  [TARGET]...           要启动的 name 或 alias，可给多个

Options:
  -c, --config <FILE>   指定 TOML 配置文件
  -t, --track           等待子进程结束并汇报退出状态
  -p, --path <PROGRAM>  直接启动某个程序（可重复指定）
  -h, --help
  -V, --version
```

### 示例

```bash
runf code                  # 按别名启动
runf obs blender           # 一次启动多个
runf -p C:/Windows/notepad.exe    # 绕过配置直接启动
runf -t runf-test          # 启动并等待，报告退出码
runf list                  # 列出全部条目
runf search editor         # 搜索
runf search editor rust    # 多个关键词为「与」关系，需同时命中
```

### `search` 与「启动」的匹配方式不同

- **启动**（`runf <TARGET>`）：`name` / `alias` **精确匹配**（大小写不敏感）
- **搜索**（`runf search <KEYWORD>`）：对 name / alias / keywords / **path** / description 做**子串匹配**

所以偶尔会看到「搜索结果是另一个条目」——那只是子串命中了它的某个字段，**不影响启动**。
例如某条目 keywords 里有 `flowchart`，那么 `search fl` 也会把它列出来。

## 工作目录：为什么需要它

有些程序会**相对自身安装目录**去找资源（本地化文件、插件、皮肤）。Windows 快捷方式默认把
「起始位置」设为程序所在目录，所以双击没事；但直接 `CreateProcess` 启动时，子进程会继承
**启动者**的工作目录，于是这类程序会报错或直接闪退。

RunFast 的默认行为与快捷方式一致：**未显式指定 `work_dir` 时，取被启动程序自身所在目录**。
若某程序需要别的工作目录，显式写 `work_dir` 覆盖即可。

> 边界情况：`path` 写的是裸文件名（靠 `PATH` 查找）时，没有「自身目录」可取，
> 此时保持继承调用者的工作目录。

## 常见问题

**Q：`cargo build --release` 时 rustc 崩溃（`0xc0000005` / `STATUS_ACCESS_VIOLATION`）？**
多半是 fat LTO 在特定工具链/环境下的问题。把 `Cargo.toml` 里 `[profile.release]` 的

```toml
lto = true
```

改为

```toml
lto = "thin"
```

即可正常编译（产物略大一点，功能无差别）。

**Q：改了配置要重新编译吗？**
不用。配置是运行时读取的普通文本文件。

**Q：启动报 `unknown target 'xxx'`？**
该名字不在配置里。`runf list` 看看实际有哪些；注意匹配是**精确**的，不是模糊。

**Q：启动报 `target 'xxx' has no usable program path`？**
`path` 里所有候选都启动失败了，错误信息里会带上每个候选的具体失败原因。

## 许可

[MIT](LICENSE)
