# 原生 DSH 终端

[English](README.md) | 中文

Rust 可执行文件 `dsh` 无需参数即可在当前项目中打开编程会话。Ratatui 界面提供固定的多行输入框、可滚动对话、流式回答、代码块、工具结果和独立的权限提示。它负责终端编辑、内存历史、括号粘贴和中断处理。本地 Node.js Agent 通过现有 `headless` 配置使用带版本的逐行 JSON 传输，不启动 Web 服务。这是原生前端，并非将 Agent 引擎改写为 Rust。

## 安装

安装 Rust 1.88 或更新版本、Cargo 和 Node.js `^22.19` 或 `>=24`，然后构建此分支并安装可执行文件：

```sh
pnpm install
pnpm run build
./install.sh
```

安装器默认使用 `~/.local/bin/dsh`；通过 `DSH_INSTALL_PREFIX` 选择其他前缀。如果安装器提示缺少路径，请将该前缀的 `bin` 目录加入 `PATH`。Cargo 不会覆盖无关的已有 `dsh` 程序。用 `command -v dsh` 检查 shell 实际选择的版本。

请将构建后的仓库保留在安装位置：程序记录了 `apps/cli/lib/bin.js` 的路径。移动仓库后需要重新安装，或将 `DSH_RUNTIME_JS` 指向新入口。运行时仍需要 Node 和仓库依赖。`DSH_NODE` 可指定 Node 程序。上游 npm 发布不包含此分支的修改。Shell 安装器面向 Linux/macOS；Windows 上构建运行时后使用 `cargo install --locked --path native/cli`。

## 使用

```sh
cd /path/to/project
dsh
dsh --resume session-…
dsh --cwd /path/to/another-project
dsh headless "run the tests"
dsh web
dsh --demo
```

也可在项目 `.env` 中设置 API 密钥。`/help`、`/session`、`/multiline`、`/exit`、审批和持久化遵循[终端参考](../../packages/bundle/headless/README.zh.md#interactive-terminal)。方向键用于编辑和浏览最多 100 条内存提示历史；审批回答不会进入历史。括号粘贴保留多行文本，等待 Enter 提交。Ctrl+C 取消运行中的轮次，或在提示符处退出；Ctrl+D 关闭会话。管道任务使用显式 `headless` 命令。显式配置参数原样转交 Node 启动器。`dsh --version` 显示前端版本；`dsh headless --version` 显示 Agent 运行时版本。

`dsh --demo` 无需 Node、凭据或模型请求即可预览界面。在终端中按 1 查看欢迎页、2 查看示例对话、3 查看批准提示；Esc 退出。重定向输出时打印静态预览。此模式中的工具结果为测试示例。

| 按键 | 操作 |
|---|---|
| Enter | 发送消息或回答 |
| Alt+Enter / Ctrl+J | 插入换行 |
| PgUp / PgDn | 滚动对话或诊断信息 |
| Ctrl+End | 返回最新消息 |
| Ctrl+T | 展开工具参数和结果 |
| F1 | 打开快捷键帮助 |
| F2 / `/diagnostics` | 查看运行时警告和错误 |
| `/` 或 Ctrl+P，随后 ↑ / ↓ 和 Enter | 选择并执行命令；Tab 补全 |
| `/clear` | 清空显示，保留 Agent 上下文 |

界面使用备用屏幕，退出时恢复终端。运行时诊断信息可通过 F2 查看；启动失败时，在恢复 shell 后打印诊断信息。显示至少需要 28 列、10 行，建议使用 80 × 24 或更大的终端。

<a id="connect-and-configure"></a>

## 连接和配置

在 `dsh` 中运行 `/login`，将 DeepSeek API key 粘贴到隐藏输入框。Enter 保存，Esc 取消。密钥不会进入屏幕输出、输入历史、Agent 消息或 Session 日志。它通过现有[凭据提供者](../../packages/credentials/credentials-local/README.zh.md)保存到 `$DSH_HOME/.credentials.yaml`（POSIX 权限 `0600`），并非加密的系统钥匙串；以同一用户身份运行的进程可以读取。此功能使用 API key 认证，不是浏览器账户登录。

| 命令 | 操作 |
|---|---|
| `/login` | 通过隐藏输入保存或替换 DeepSeek API key |
| `/logout` | 确认删除已保存密钥，并报告环境或 `.env` 回退来源 |
| `/status` | 显示工作区、当前模型、凭据是否存在及其来源 |
| `/model` | 为当前会话后续提示选择一个已公布的模型 |
| `/check` | 通过当前模型发送简短连接检查提示；按正常 API 计费 |
| `/sessions` | 列出当前项目最多十个已保存会话及恢复命令 |

不要将密钥作为斜杠命令参数。`/login` 管理 `DEEPSEEK_API_KEY`；自定义提供者凭据引用仍通过现有环境或设置配置。保存密钥不代表已验证，只有 API 请求成功才能验证。模型选择仅在当前进程有效；后续启动使用配置的默认模型。

<a id="development"></a>

## 开发

```sh
cargo test --locked --manifest-path native/cli/Cargo.toml
cargo clippy --locked --manifest-path native/cli/Cargo.toml --all-targets -- -D warnings
cargo build --locked --manifest-path native/cli/Cargo.toml
python3 -m pip install -r native/cli/tests/requirements.txt
python3 native/cli/tests/terminal_smoke.py
```

POSIX PTY 测试使用假运行时，不发送 API 请求，并通过 Pyte 检查终端显示。源码开发时，先构建宿主系统插件、生成 Typert 文件，再从同一仓库版本重新构建原生程序：

```sh
node --import tsx native/system/scripts/build.ts --host-addon-only
node --import tsx scripts/generate-typert.ts
cargo build --release --locked --manifest-path native/cli/Cargo.toml
```

随后将 `DSH_RUNTIME_JS` 设为 `apps/cli/src/bin.ts` 的绝对路径，并设置 `NODE_OPTIONS='--import tsx/esm'`，同时安装源码依赖。

前后端使用终端协议版本 3。更新任一侧后，应一并重建两侧。缺少 `lib/typert.host.js` 表示尚未生成运行时文件；源码开发使用上述生成命令，已安装的运行时使用 `pnpm run build`。

缺少 `system.node` 表示尚未构建宿主系统插件；请运行上面的 host-addon 命令。该插件提供保存和恢复 Session 所需的文件锁。后端失败时，前端恢复终端、打印诊断信息，并以非零状态退出。
