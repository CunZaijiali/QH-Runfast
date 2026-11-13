# RunFast (`runf`)

> Start configured programs quickly —— 按名字或别名快速启动本机程序的小工具。

把「一长串绝对路径」变成「一个短名字」。配置文件是纯 TOML，改起来不用重新编译。

```
$ runf code        # 启动 VS Code
$ runf obs         # 启动 OBS Studio
$ runf list        # 看看都配了些什么
$ runf search sql  # 按关键词找
```

## 安装

**从 crates.io 安装（推荐）**

```bash
cargo install run-fast
```

安装后命令名是 **`runf`**（包名是 `run-fast`，二进制名是 `runf`）。

**从源码安装到本地**

```bash
git clone https://github.com/CunZaijiali/QH-Runfast
cd QH-Runfast
cargo install --path .
```

**只构建、不安装**

```bash
cargo build --release
```

产物是 `target/release/runf.exe`（包里定义了 `[[bin]] name = "runf"`），把它放进任意已在 `PATH` 上的目录即可，例如：

```powershell
Copy-Item .\target\release\runf.exe "$env:LOCALAPPDATA\Programs\runf\runf.exe"
```

需要 Rust **1.85+**（edition 2024）。

## 特性

- **按名字 / 别名启动**：大小写不敏感，自动去掉首尾空格
- **多路径回退**：一条目可写多个候选路径，按顺序尝试，第一个成功即用
- **关键词搜索**：`search` 会在 name / alias / keywords / path / description 里找
- **工作目录自动处理**：未显式指定时，自动把子进程的工作目录设为程序自身所在目录
  （等价于 Windows 快捷方式的「起始位置」，可解决 OBS 这类程序找不到自身 `locale`/`data` 的问题）
- **参数透传**：`--args "..."` 在配置的启动参数之后追加临时参数，不必为每种参数组合建条目
- **`.bat` / `.cmd` 支持**：自动经 `cmd /S /C` 启动
- **批量启动**：一条命令可同时启动多个目标
- **进程跟踪**：`--track` 等待子进程结束并汇报退出状态
- **配置自检**：重复的 `name` / `alias` 会在启动前被拒绝，避免「配错了却不知道」

## 快速开始

1. 建一份配置（见下节），例如 `D:\GlobalScripts\bin\config.toml`
2. 确保 `runf` 在 `PATH` 上
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
  [TARGET]...            要启动的 name 或 alias，可给多个

Options:
  -c, --config <FILE>    指定 TOML 配置文件
  -t, --track            等待子进程结束并汇报退出状态
      --args <ARGS>      追加到该条目 start_args 之后的临时参数（可重复）
  -p, --path <PROGRAM>   直接启动某个程序（可重复指定）
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

### 参数透传：`--args`

配置里的 `start_args` 是固定的；临时想加参数（换镜像、加调试开关、改内存大小）不必改配置：

```bash
runf qemu --args "-m 512 -accel tcg"
runf qemu --args "-m 512" --args "-s -S"     # 可重复，按顺序累积
```

规则：

- 值按**空白**切成若干个参数，追加在 `start_args` **之后**
- 用**双引号或单引号**包裹含空格的值（引号本身会被剥掉）：
  ```bash
  runf qemu --args '-drive "format=raw,file=D:/my os/bios.img"'
  ```
- **反斜杠不是转义符**，`D:\XiangMu\QH_OS` 这类 Windows 路径原样传给子进程
- 空串或全空白视为「不追加」，行为与不带该选项完全一致
- 与 `-p/--path` 一起用时，这些参数就是该程序的全部参数（没有 `start_args` 可追加）

> 注意：从启动器（不打开控制台）启动程序时，`-serial stdio` 这类依赖标准输出的参数看不到输出，
> 需要留痕时改用文件形式，例如 `--args "-serial file:D:/tmp/serial.log"`。

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

即可正常编译（产物略大一点，功能无差别）。本仓库当前已经使用 `thin`。

**Q：改了配置要重新编译吗？**
不用。配置是运行时读取的普通文本文件。

**Q：启动报 `unknown target 'xxx'`？**
该名字不在配置里。`runf list` 看看实际有哪些；注意匹配是**精确**的，不是模糊。

**Q：启动报 `target 'xxx' has no usable program path`？**
`path` 里所有候选都启动失败了，错误信息里会带上每个候选的具体失败原因。

**Q：Windows 上跑 `cmd.exe` 报 `The directory or file cannot be created.`？**
`cmd.exe` 不接受 argv[0] 里带正斜杠的自身路径。配置里把 `path` 写成反斜杠形式
（`C:\\Windows\\System32\\cmd.exe`）即可，其他程序一般不受影响。

**Q：为什么安装后的命令叫 `runf` 而不是 `run-fast`？**
包名 `run-fast` 便于在 crates.io 上识别，二进制名由 `[[bin]] name = "runf"` 固定为短名。

## 许可

[MIT](LICENSE)
