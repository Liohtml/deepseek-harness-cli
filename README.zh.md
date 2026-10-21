# DeepSeek Harness CLI

[English](README.md) | 中文

[![界面：Rust + Ratatui](https://img.shields.io/badge/interface-Rust_%2B_Ratatui-67b7ff?style=flat-square&labelColor=171e28)](native/cli/README.zh.md) [![状态：开发者预览](https://img.shields.io/badge/status-developer_preview-f1c27b?style=flat-square&labelColor=171e28)](#developer-preview) [![许可证：MIT](https://img.shields.io/badge/license-MIT-8dd6b4?style=flat-square&labelColor=171e28)](LICENSE)

**你的项目，你的下一步。**

在终端里了解代码库、修改代码、排查测试失败。安装后，进入项目目录，输入 **`dsh`** 即可开始。

这是 [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness) 的独立分支，将原生 **Rust + Ratatui** 界面与现有 Node.js Agent 引擎结合。工具和持久化会话由 Harness 提供，工作区始终留在终端中。

[快速开始](#quick-start) · [日常工作](#daily-workflow) · [界面截图](#screenshots) · [CLI 参考](native/cli/README.zh.md) · [报告问题](https://github.com/Liohtml/deepseek-harness-cli/issues)

![DSH 终端演示：欢迎页、命令补全、示例对话、工具详情和权限提示](docs/assets/cli/dsh-demo.gif)

*录制自真实的 `dsh --demo` 界面。消息和工具结果为示例数据，没有发送模型请求。需要静态图片？[查看对话截图](docs/assets/cli/conversation.png)。*

<a id="terminal-coding-agent"></a>

## 为终端工作而设计

| 专注当前任务 | 掌握执行过程 |
|---|---|
| **流式对话** — 实时查看回答，支持代码块和可滚动的对话记录。 | **工具活动可见** — 用 `Ctrl+T` 查看工具参数、结果和失败信息。 |
| **完整的输入体验** — 多行输入、Unicode、括号粘贴和提示历史。 | **明确的批准操作** — 独立的权限提示；直接回车拒绝请求。 |
| **随手调用命令** — 输入 `/`，用方向键选择，再按 Tab 补全。 | **可恢复的会话** — 用 `/session` 获取当前 id 和恢复命令。 |

<a id="run"></a><a id="run-from-source"></a>

<a id="quick-start"></a>

## 快速开始

需要 **Rust 1.88+ 和 Cargo**、**Node.js `^22.19` 或 `>=24`**，以及 **pnpm**（版本固定在 `package.json` 中）。Shell 安装器支持 Linux 和 macOS；Windows 与自定义路径见[安装参考](native/cli/README.zh.md)。

构建并安装此分支：

```sh
git clone https://github.com/Liohtml/deepseek-harness-cli.git
cd deepseek-harness-cli
pnpm install
pnpm run build
./install.sh
export PATH="$HOME/.local/bin:$PATH"
```

进入项目并启动会话：

```sh
cd /path/to/your/project
dsh
```

在 `dsh` 中运行 `/login`，通过隐藏输入框填写 API key，再用 `/model` 选择模型。`/check` 发送简短 API 连接测试（正常计费）。密钥保存在 DSH 私有凭据文件中；参阅[存储与安全说明](native/cli/README.zh.md#connect-and-configure)。也可以使用环境变量或项目 `.env` 中的 `DEEPSEEK_API_KEY`。请保留构建后的仓库：已安装程序使用其中的 Node.js 运行时。将安装目录加入 shell 的 PATH 配置，使后续终端也能找到程序。[安装参考](native/cli/README.zh.md) 说明运行时路径，以及安装多个版本时如何选择正确的 `dsh`。

**想先体验界面？** 安装后运行 `dsh --demo`，无需 API 密钥。按 `1` 查看欢迎页、`2` 查看示例对话、`3` 查看权限提示，按 Esc 退出。

<a id="daily-workflow"></a>

## 日常工作

```sh
dsh                                      # Work in the current project
dsh --cwd /path/to/project               # Open another project
dsh --resume session-…                   # Continue a saved session
dsh headless "Run tests and summarize failures"  # One task, then exit
```

从具体任务开始：

> 解释这个项目的组织方式，以及请求从哪里进入系统。

> 找出测试失败的原因，做有针对性的修复，并运行相关检查。

> 检查尚未提交的修改，寻找 bug 和缺失的错误处理。

用 `/session` 查看恢复命令，输入 `/exit` 关闭会话。恢复时使用同一个项目目录。脚本和管道应显式选择 `headless`；交互界面需要真实终端。

### 常用快捷键

| 按键 | 操作 |
|---|---|
| Enter | 发送消息或回答 |
| Ctrl+P 或 `/` | 浏览命令；↑ / ↓ 选择，Enter 执行 |
| Alt+Enter / Ctrl+J | 插入换行 |
| ↑ / ↓ | 浏览提示历史 |
| PgUp / PgDn | 滚动对话 |
| Ctrl+T | 展开或折叠工具详情 |
| F1 / F2 | 打开快捷键帮助 / 运行时诊断 |
| Ctrl+C | 停止当前轮次；空闲时退出 |
| Ctrl+D | 关闭会话 |

命令、权限和输入行为的详情见[完整终端参考](native/cli/README.zh.md)。

<a id="screenshots"></a>

## 界面截图

以下图片与 GIF 来自同一个离线演示。点击图片查看完整分辨率。

<details>
<summary><strong>欢迎页 — 工作区、模型与下一项任务</strong></summary>

[![DSH 欢迎页：工作区信息、建议任务和消息输入框](docs/assets/cli/welcome.png)](docs/assets/cli/welcome.png)

</details>

<details>
<summary><strong>对话 — 回答、代码与工具活动</strong></summary>

[![DSH 示例对话：已完成的工具、Rust 代码块和固定消息输入框](docs/assets/cli/conversation.png)](docs/assets/cli/conversation.png)

</details>

<details>
<summary><strong>权限 — 批准前先检查请求</strong></summary>

[![DSH 权限提示：请求执行的命令和琥珀色回答输入框](docs/assets/cli/approval.png)](docs/assets/cli/approval.png)

</details>

<a id="developer-preview"></a>

## 开发者预览

此分支处于**开发者预览**阶段，界面和兼容性可能变化。在项目中运行 Agent 前，请阅读[安全说明](SAFETY.zh.md)。

前端使用原生 Rust；运行 Agent 仍需要 Node.js 和构建后的仓库。上游 npm 包不包含此分支的终端界面。遇到启动问题时，按 F2 打开诊断，并查看[构建与运行时故障排查](native/cli/README.zh.md)。

## 其他界面

<details>
<summary>使用 Web UI 或 Node.js 前端</summary>

在构建后的仓库中，`pnpm dsh` 打开普通 Node.js 终端前端；`pnpm dsh web` 无需重建即可启动 Web UI。安装 Rust 程序后，`dsh web` 也会转发到 web 配置。

<a id="run-from-npm"></a>

通过 npm 体验上游 Web UI：

```sh
npx @deepseek-ai/dsh web
```

默认服务地址为 `http://127.0.0.1:3080`，本地启动时打开浏览器。SSH 启动仅打印 URL，端口转发由 SSH 客户端或编辑器处理。传入 `--no-open` 可阻止打开浏览器。见 [Web UI 指南](docs/user/guide/index.zh.md)。

</details>

## 社区与支持

- **此 CLI：**[bug 报告和功能请求](https://github.com/Liohtml/deepseek-harness-cli/issues)。
- **上游 Harness：**[Discussions](https://github.com/deepseek-ai/deepseek-harness/discussions) 和 [Discord](https://discord.gg/4MrtZUhpxg)。
- **插件：**添加 [`dsh-plugin`](https://github.com/topics/dsh-plugin) 话题，帮助其他人发现你的扩展。

## 参与贡献

请先阅读 [CONTRIBUTING.md](CONTRIBUTING.zh.md)、[开发指南](docs/development.zh.md) 和[原生前端指南](native/cli/README.zh.md)。Agent 应阅读 [AGENTS.md](AGENTS.md)。README 媒体可通过[录制脚本](native/cli/tools/capture_demo.py)重新生成。

<a id="development"></a>

## 基于 DeepSeek Harness

上游 Harness 由 [DeepSeek AI](https://deepseek.com) 开发。其**一切皆插件**架构由 [Cordis](https://github.com/cordiverse/cordis) 驱动，设计详见 [*A Programming Paradigm for Spatiotemporal Composability*](https://arxiv.org/abs/2608.25512)。了解底层引擎可阅读[架构文档](docs/architecture.zh.md)和[上游文档](https://deepseek-harness.github.io/deepseek-harness/)。

<a id="citation"></a>

<details>
<summary>引用上游项目</summary>

```bibtex
@misc{deepseek-harness2026,
  title={DeepSeek Harness: Everything is a Plugin},
  author={DeepSeek-AI},
  year={2026},
  publisher={GitHub},
  howpublished={\url{https://github.com/deepseek-ai/deepseek-harness}},
}
```

</details>

## 许可证

[MIT](LICENSE)。依赖许可证见[第三方声明](THIRD_PARTY_NOTICES.md)。
