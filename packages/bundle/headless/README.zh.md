---
description: "dsh 的一次性任务模式：从命令行运行单个任务并打印最终答案，供用户脚本化或自动化 dsh。"
kind: "package-bundle"
---

# @deepseek-ai/dsh-headless

[English](README.md) | 中文

## 概述

`dsh-headless` 从命令行运行一个 dsh 任务并打印最终答案，然后退出——没有 GUI、没有服务器、没有浏览器。输入 `dsh --profile headless "run the tests"`，agent（智能体）会以与所有其他表层相同的模型、工具与安全默认值完成该任务。它非常适合脚本、CI 与一次性任务：进程不打开任何端口，也不会留下任何后台运行的东西。监督进程还可以通过按行 JSON 事件流（`--json`）驱动它，并用该事件流报告的标识（`--session-id`）在同一段对话上继续唤醒。退出码告诉你结果——任务完成时为 0，中止或出错时为 1。使用 `--interactive` 可在终端进行多轮对话。

## 目录

- [使用本包](#use-this-package)
- [理解实现](#understand-the-implementation)
- [进一步探索](#further-exploration)
- [模型体验](#model-experience)
- [已知限制与延期工作](#known-limitations-and-deferred-work)
- [开发备注](#dev-note)

-----

<a id="use-this-package"></a>
## 使用本包

运行一个任务，获得最终答案，然后退出。任务就是命令行参数，省略时则来自 stdin；整条命令就是最小的可运行示例。

### 运行一次性任务

```sh
dsh --profile headless "run the tests"
```

agent 会完成该任务，把提供方的每个非空推理（reasoning）增量流式写入 stderr 的 `dsh: reasoning:` 段，然后把最终答案写入 stdout 并退出。连续推理增量保持在同一段中；提供方未给尾换行时，runner 会在后续输出前结束该段。没有推理内容的成功运行保持 stderr 为空；失败时退出码为 1，并以 `dsh: <code>: <message>` 向 stderr 写入错误。任务来自位置参数，参数省略或为单独的 `-` 时则来自 stdin；空白位置参数或空管道会在任何执行开始之前被拒绝。位置参数会原样作为任务，stdin 不会被读取，因此想让管道内容进入提示词时，请把完整提示写进管道；管道任务会原样发送，包括结尾换行。

```sh
{ echo "Summarize these changes:"; git diff --stat; } | dsh --profile headless
```

任务与运行选项通过以下设置提供：

| 字段 | 默认值 | 含义 |
|---|---|---|
| `task` | stdin | 任务文本；省略或传 `-` 时由 stdin 提供 |
| `sessionId` | `session-<uuid>` | 要沿用的精确 Session 标识；未知 id 会失败 |
| `json` | `false` | 把本次运行投影为 stdout 上的按行 JSON 事件 |
| `interactive` | `false` | 接受终端提示，直到 EOF 或 `/exit` |
| `terminalBridge` | `false` | 原生前端的 JSON stdio 传输；要求 `interactive` |

生成的[配置目录](../../../docs/config-catalog.zh.md#deepseek-aidsh-headless)是所有受支持字段及其 JSDoc 的完整真源。

<a id="interactive-terminal"></a>
### 交互式终端

```sh
pnpm dsh headless --interactive
pnpm dsh headless --interactive "Inspect this project's test failures"
pnpm dsh headless --interactive --session-id session-…
```

从项目目录启动；仓库源码命令使用该目录作为工作区。交互模式要求 stdin 和 stdout 均为终端，并拒绝 `--json` 和 stdin 标记 `-`。模型、文件工具、沙箱、凭据和持久化 Session 与一次性任务相同。Session 在显示 id 和首个提示之前刷新到持久存储，因此空对话也能恢复。每轮在接受下一个提示之前刷新到持久存储。agent 工作时显示已提交的助手消息、工具名称和参数；普通终端不显示推理和未完成的 token 增量。原生前端流式显示文本增量，并以已提交的消息替换对应的部分回答；始终不显示推理。

| 输入 | 操作 |
|---|---|
| `/help` | 显示终端命令 |
| `/session` | 打印当前 id 和恢复命令 |
| `/multiline` | 输入多行原始文本；单独的 `.` 提交 |
| `/exit`、`/quit` 或 Ctrl+D | 结束对话 |
| 运行中按 Ctrl+C | 取消当前轮并返回提示符 |
| 提示符处按 Ctrl+C | 退出 |

批准提示仅接受 `y` 或 `yes` 作为一次性授权；直接回车拒绝。agent 问题接受选项编号或自由文本，多选题还接受逗号分隔的编号。取消提示不会授予批准。正常退出返回 0，即使此前某轮被取消或失败；意外的驱动或持久化失败终止进程并返回 1。脚本需要任务结果作为退出码时，使用一次性模式。

终端只回答根 agent 的请求。限时问题保留现有超时；此终端模式不支持超时后的答复、会话浏览、模型切换和子 agent 交互。恢复会话遵循与一次性模式相同的 cwd 和预设限制。

`dsh` 无参数时启动交互模式。[Rust 前端](../../../native/cli/README.zh.md) 通过 `--terminal-bridge` 将终端输入与 Agent 进程分离。此选项要求专用 JSON stdio，不能与 `--interactive`、`--json` 或任务文本组合。版本 2 使用 `hello`、`session`、`assistant_start`、`assistant_delta`、`assistant`、`tool`、`tool_result`、`output`、`prompt`、`cancel`、`close` 和 `error` 帧；客户端发送 `answer`（含当前提示 id）、`interrupt` 或 `close`。EOF 会取消当前轮次。

<a id="credentials"></a>
### 凭据

随附的 headless profile 禁用托管凭据存储，以及依赖它的 authorization 和 DeepSeek account 消费方。API 密钥路由依次使用启动环境、`<运行命令的目录>/.env` 和 `$DSH_HOME/.env`。GUI 保存的密钥和登录凭据不会被使用，存储文件保持原样。需要已保存登录凭据的提供方会失败，除非恢复存储认证；AWS profile 等提供方外部凭据链仍然可用。

启动变量为空时，请取消导出该变量，让 `.env` 后备生效；已导出的空值会遮蔽后备值。

要恢复存储认证，请把以下行加入 `$DSH_HOME/profiles/headless/cordis.patch.yml` 或 `--patch` 覆盖文件：

```yaml
- id: credentials
  disabled: false
- id: authorization
  disabled: false
- id: deepseek-account
  disabled: false
```

恢复后，存储仍按[已保存凭据优先于环境后备](../../credentials/credentials-local/README.zh.md)解析。Web、SDK 和 ACP profile 保留这一默认顺序。headless 的会话、settings 和工具行为不变；这种 profile 配置选择不构成文件系统访问边界。

### 选择 Session 标识

每次调用默认使用全新的 `session-<uuid>` 标识，`--json` 会在开头的 `session` 事件里报告它。传入 `--session-id <id>` 延续这段对话：runner 沿用该 id 对应的持久化 Session，而该 id 没有持久化 Session 时会在任务运行前失败，而不是悄悄开出一段空历史。沿用要求已组合 `sessionPersistence` 与 `sessionQuery` 服务，因此缺少任一服务的 profile 会显式失败，而不会返回一个历史随进程消失的 id。本进程中已存在持有该 id 的存活 Agent 时会被拒绝：它的原 owner 可能仍在驱动它，runner 无法取得独占的运行区间。标识是不透明的，因此会原样使用调用方给出的字符串，包括空白字符。工作目录通过已挂载的文件系统提供方解析（`fs.resolve('.')` 与 `fs.processPath()`）；未挂载文件系统服务时使用进程 cwd，新 Session 会记录该目录。沿用会将已记录 cwd 与同一提供方解析出的目录比较，并拒绝子 agent 或 fork 会话、未记录工作目录的会话、运行在本 profile 不组合的 agent preset 下的会话，以及 preset 记录畸形的会话——该检查读取 Session 日志当前记录的 preset，因此在空白期切换过 preset 的会话同样会被拒绝。因此监督进程无法在另一套组合下悄悄驱动他人的会话；任一不匹配都会在任务运行前失败。

### 机器可读输出

`--json` 用按行 JSON 事件流取代 stdout 的最终文本行，stderr 仅保留 `dsh:` 诊断信息。事件流以 `session`（携带本次运行使用的标识）开头、以 `final` 结尾，其间为 `status`、`text`、`thinking`、`tool_call` 与 `tool_result` 事件。`text` 与 `thinking` 只从已提交的 assistant 消息投影，因此被重试或丢弃的尝试不会进入事件流；它们在步骤提交时到达，而不是逐 token 到达，默认模式的 stderr 推理仍是唯一的实时文本通道。终止 `final` 事件携带与默认模式相同的无损答案，不做限长；其他每个字符串与对象键上限为 8 KiB，超出时标记 `truncated`，单条事件行（含换行）上限为 32 KiB——超长事件保留标量字段、丢弃结构化字段，极端情况下只剩 `type` 与 `truncated`，嵌套达到 64 层及以上的负载会在该深度被截断。空工具参数字符串会投影为 `{}`，与执行器实际运行的值一致；而 JSON 无法往返的参数——例如溢出为 `Infinity` 的数字 `1e400`——会保留原始文本，而不是 `JSON.stringify` 会报告的 `null`。runner 在轮次之外抛出的失败会写出 `error` 事件并在没有 `final` 的情况下结束事件流，同时向 stderr 写入 `dsh:` 行；若 profile 自身的插件加载失败，进程会在 runner 挂载前退出，该情形只保留 loader 的 stderr 诊断。轮次内失败的运行仍会以 `final` 事件（通常为空）结束且没有 `error` 事件，因此格式良好的事件流也可能描述一次失败的运行：请把退出码 1 与 `turn_end` 原因作为失败信号。

### 何时使用

在脚本化或自动化的 dsh 运行中使用 headless——CI 步骤、批处理任务、从终端快速获取答案。多轮终端工作使用 `--interactive`；需要 GUI 时使用浏览器表层（[dsh-web-app](../web-app/README.zh.md)）。进程只为本次运行而存活，不打开监听端口，并且自行退出，因此适合等待进程结束的流水线。当监督进程需要进度而不只是答案时，`--json` 提供事件流，`--session-id` 则让后续调用继续同一段对话。

### 帮助与任务错误

`dsh --profile headless --help` 打印该命令的帮助文本并直接退出，不运行任何内容。只有空白的位置参数本身就属于用法错误——什么都不运行，进程退出 1，即使 stdin 不是终端也一样，因此误传的空白参数绝不会消费管道内容。完全缺失任务时，仅在 stdin 是终端时属于用法错误；stdin 不是终端时，runner 改从 stdin 读取任务，并以同样方式拒绝空结果。单独的 `-` 是唯一的 stdin 标记；把它与其他任务词混用属于用法错误，而不是以连字符开头的任务。在 `--json` 模式下，所有用法错误——包括 commander 自身的语法拒绝，例如未知选项或选项缺少取值——都会在进程退出前向 stdout 写出 `error` 事件，因此按行解析的监督进程即使 runner 从未挂载也能看到格式良好的事件流；事件 `message` 中的文本不带 commander 的 `error: ` 前缀。

-----

<a id="understand-the-implementation"></a>
## 理解实现

<details>
<summary>实现细节——点击展开</summary>

runner 是核心 API 载体之上的直接驱动器：它确定 Agent 标识——默认是全新的 `session-<uuid>`，或 `--session-id` 指名的持久化 Session——并把所属的持久化事件区间折叠成一个进程级结果。

### 运行流程

runner 等待整个应用结算（`ctx.get('loader')?.await()`），确保已组合的工具与适配器不会半挂载，读取共享的 [`agentDefaultModel`](../../core/agent-default-model/README.zh.md) 选择，从配置或 stdin 解析任务，然后确定 Agent 标识：默认是全新的 `session-<uuid>`，或是 `--session-id` 指名的持久化 Session——通过 [`sessionQuery`](../../session-query/session-query/README.zh.md) 沿用，日志不存在时拒绝。它把任务作为普通用户消息提交。不带 `--json` 时，它把该 Agent 的非空推理增量流式写入 stderr；带 `--json` 时改为投影本次运行。它等待子级工作，并在结束前复核父级仍空闲且 Session 序号未变，让结算通知可以驱动父级生成最终答案。收件箱消息处于停放状态的空闲子级不会延迟完成；进程关闭时丢弃这些未领取消息。随后它对会话执行 flush，并把所属区间（从 `firstSeq` 起）折叠为最后一条非空 `assistant/message` 文本与最终 `turn/end` 原因。最后，它把最终文本写入 stdout（或 `final` 事件）并请求退出。

### 基于 base 的 patch 内容

patch 叠加在 `dsh-base` 之上：继承投影缓存与共享 PTC 运行时，在基础 `system-prompt` 行上设置编码 persona 前缀与独立的 cwd 后缀，保留与 Web 表层相同的临时进程级 PTC mode 开关（`DSH_TOOLS_MODE`），禁用共享的 HMR（热模块替换）行，并挂载启动提供方与 runner。缓存为每个已持久化的一次性会话写入检查点，供后续消费方使用；其持久性屏障会在发布缓存行前 flush 所覆盖的日志前缀，因此可能拆分原本会合并的 JSONL 连续段。启动提供方（[`src/startup.ts`](src/startup.ts)）注入 `ctx.cmdlineArgs`（[`dsh-cmdline`](../../boot/cmdline/README.zh.md)），读取位置参数与 `--session-id`/`--json`/`--interactive` 选项、打印应用自己的 `--help`，并提供 `headlessStartup`；runner 注入该服务，再从惰性配置中读取任务与运行选项。

### 退出映射

最终 `turn/end` 完成时退出码为 0；任何其他结果——aborted、error，或所属区间内没有轮次——退出码为 1。结束原因为 `error` 时还会向 stderr 写入 `dsh: <code>: <message>`。若实时 Agent 失败后没有提交更晚的 `turn/end`，则视为直接驱动器失败，包括汇总轮次的开始或结束无法持久化；更早的已完成轮次不能使该次运行成功。直接驱动器失败（例如 Agent 创建失败或不可用的 `--session-id`）向 stderr 写入 `dsh: <message>` 并退出 1，且在 `--json` 模式下额外发出一个 `error` 事件，不发出 `final` 事件。

### 源码地图

| 文件 | 职责 |
|---|---|
| [`src/index.ts`](src/index.ts) | `headless-runner` 插件：运行流程、Session 解析、输出约定、退出映射 |
| [`src/startup.ts`](src/startup.ts) | `headless-startup` 提供方：任务位置参数、`--session-id`、`--json` 与 `--help` |
| [`src/json-stream.ts`](src/json-stream.ts) | `--json` 投影：事件词汇、提交点发射、字符串限长 |
| [`cordis.patch.yml`](cordis.patch.yml) | 叠加在 `dsh-base` 之上的一次性 patch |
| [`tests/headless.spec.ts`](tests/headless.spec.ts) | 运行流程、汇总、flush、Session 沿用与退出映射 |
| [`tests/json-stream.spec.ts`](tests/json-stream.spec.ts) | 投影顺序、提交点发射、限长与释放 |
| [`tests/startup.spec.ts`](tests/startup.spec.ts) | 在真实 Loader 树上的命令行解析 |

</details>

-----

<a id="further-exploration"></a>
## 进一步探索

当你想深入了解共享核心、同级 GUI 或命令行交接时，阅读以下页面。

- [组合包索引](../README.zh.md)——基于同一核心构建的表层。
- [dsh-base](../base/README.zh.md)——headless 运行其上的共享核心。
- [dsh-web-app](../web-app/README.zh.md)——用于多轮工作的同级交互式浏览器入口。
- [dsh-cmdline](../../boot/cmdline/README.zh.md)——启动器如何把命令行交给应用。
- [生成配置目录](../../../docs/config-catalog.zh.md#deepseek-aidsh-headless)——每个受支持配置字段及其源声明。

-----

<a id="model-experience"></a>
## 模型体验

无，因为 runner 把任务作为普通用户消息提交，提示词与工具由组合出的 base 与 headless 行提供。

#### KV Cache 影响

runner 不向请求前缀添加任何内容；它只是驱动组合出的配置树处理一条用户消息。

## 已知限制与延期工作

<a id="known-limitations-and-deferred-work"></a>


这些限制告诉你 headless 何时不适用、它需要 `dsh` 启动器提供什么。它们是当前包约束，不是通用的 CLI（命令行界面）对比或任务积压。

- **终端模式按行输入**——`--interactive` 支持后续对话，但没有全屏编辑器或会话浏览器。
- **持久化历史可能缺少后台 job 的最后一条结算**——进程销毁可能在有主 job 结算前停止 Session 路由或关闭写句柄，正常结束和信号关停也可能如此；[job 生命周期约定](../../../docs/subsystems/jobs.zh.md#session-lifecycle-records)说明消费方如何保留不完整区间。
- **通过 `dsh` 启动器运行**——以其他方式启动 headless profile 会在启动时失败，因为只有启动器能请求进程退出。
- **首个 token 前没有心跳**——默认模式下，提供方发出第一个非空推理增量前，stderr 保持静默；延迟首个 token 的提供方不会更早给出进度信号。
- **推理进入 stderr 日志**——默认模式下，重定向与监督进程可能保留显著更多且可能敏感的模型输出；需要时应把 stderr 路由到受控位置。
- **默认 stdout 只承载最终答案**——没有 assistant 消息的运行向 stdout 打印空行并以 1 退出；中间工具输出不会打印，除非显式启用 `--json`。
- **沿用受 cwd、归属与 preset 限制**——`--session-id` 会拒绝记录在其他工作目录、未记录工作目录、属于子 agent 或 fork 会话，或运行在本 profile 不组合的 agent preset 下的 Session、preset 记录畸形的 Session，并要求已组合的 Session 查询与持久化服务；本进程中已存活的身份同样会被拒绝，因为 runner 无法对它取得独占的运行区间。
- **事件流是投影而非日志**——`--json` 除终止 `final` 外把每个字符串与对象键限制在 8 KiB，并省略投影未建模的事件，因此它不是 Session 日志的无损副本。

<a id="dev-note"></a>
### 开发备注

<details>
<summary>维护者的工作上下文——点击展开</summary>

无。

</details>
