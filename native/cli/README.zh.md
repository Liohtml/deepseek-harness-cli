# 原生 DSH 终端

[English](README.md) | 中文

Rust 可执行文件 `dsh` 无需参数即可在当前项目中打开编程会话。它负责终端编辑、内存历史、括号粘贴、显示和中断处理。本地 Node.js Agent 通过现有 `headless` 配置使用带版本的逐行 JSON 传输，不启动 Web 服务。这是原生前端，并非将 Agent 引擎改写为 Rust。

## 安装

安装 Rust、Cargo 和 Node.js `^22.19` 或 `>=24`，然后构建此分支并安装可执行文件：

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
export DEEPSEEK_API_KEY=your-key
dsh
dsh --resume session-…
dsh --cwd /path/to/another-project
dsh headless "run the tests"
dsh web
```

也可在项目 `.env` 中设置 API 密钥。`/help`、`/session`、`/multiline`、`/exit`、审批和持久化遵循[终端参考](../../packages/bundle/headless/README.zh.md#interactive-terminal)。方向键用于编辑和浏览最多 100 条内存提示历史；审批回答不会进入历史。括号粘贴保留多行文本，等待 Enter 提交。Ctrl+C 取消运行中的轮次，或在提示符处退出；Ctrl+D 关闭会话。管道任务使用显式 `headless` 命令。显式配置参数原样转交 Node 启动器。`dsh --version` 显示前端版本；`dsh headless --version` 显示 Agent 运行时版本。

## 开发

```sh
cargo test --locked --manifest-path native/cli/Cargo.toml
cargo clippy --locked --manifest-path native/cli/Cargo.toml --all-targets -- -D warnings
cargo build --locked --manifest-path native/cli/Cargo.toml
python3 native/cli/tests/terminal_smoke.py
```

POSIX PTY 测试使用假运行时，不发送 API 请求。源码开发时，将 `DSH_RUNTIME_JS` 设为 `apps/cli/src/bin.ts` 的绝对路径，并设置 `NODE_OPTIONS='--import tsx/esm'`，同时安装源码依赖。
