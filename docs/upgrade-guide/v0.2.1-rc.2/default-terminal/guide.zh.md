---
kind: upgrade-guide
description: "不带参数的 dsh 现在启动交互式编程会话，不再因缺少配置名称而退出。"
---

# `dsh` 默认启动终端 Agent

[English](guide.md) | 中文

## 变更

在此分支中，不带参数的 `dsh` 启动 `headless --interactive`，此前会报缺少配置名称并退出。显式配置命令含义不变。单独安装的 [Rust 前端](../../../../native/cli/README.zh.md) 通过 `headless --terminal-bridge` 打开同一会话；其 `--version` 显示前端版本。

## 迁移

1. 在交互终端中使用 `dsh`。自动化仍须显式选择配置，例如 `dsh headless "run the tests"`；无参数管道调用会失败并提示终端用法。
2. 用 `dsh --help` 查看启动器帮助。使用 Rust 前端时，运行 `dsh headless --version` 查询运行时版本。
3. 在项目中启动 `dsh` 并输入 `/exit`，无需模型请求即可验证启动和正常关闭。

4. Rust 前端现使用全屏 Ratatui 界面和终端协议版本 2。请一并重新构建 Node 运行时并安装 Rust 程序；版本 1 程序无法连接新版桥接。需要 Rust 1.88 或更新版本。用 `dsh --demo` 无需 API 密钥即可预览界面。源码启动前，请运行 `node --import tsx scripts/generate-typert.ts` 生成 Typert 文件。
