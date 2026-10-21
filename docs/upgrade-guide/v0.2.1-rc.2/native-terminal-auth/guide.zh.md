---
kind: upgrade-guide
description: "原生终端协议 3 要求一起重建 Rust 前端和 Node 运行时，以支持隐藏凭据输入。"
---

# 原生终端认证

[English](guide.md) | 中文

## 变更

原生终端传输使用协议 3 提供凭据状态和隐藏输入。协议 2 前后端不能与协议 3 混用。版本不匹配时，在接收凭据前拒绝握手。现有 Session 日志和已存凭据保持不变。

交互式 headless 会话现在优先读取托管凭据存储，再使用环境后备。一次性任务仍从环境选择凭据。

## 迁移

1. 在更新后的仓库运行 `pnpm install`、`pnpm run build` 和 `./install.sh`，一起重建两个组件。源码开发参阅[原生构建说明](../../../../native/cli/README.zh.md#development)。
2. 重新启动 `dsh`。运行不带参数的 `/login`，将 API key 粘贴到隐藏输入框；不要将密钥放在命令行中。
3. 确认 `/status` 显示密钥已配置。`/check` 发送简短模型请求验证访问，按正常 API 计费。存储保护和删除方式参阅[终端参考](../../../../native/cli/README.zh.md#connect-and-configure)。
4. 如需使用环境密钥代替已保存的 DeepSeek 密钥，运行 `/logout` 并确认删除。`/status` 显示剩余的凭据来源。
