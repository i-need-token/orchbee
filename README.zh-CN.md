<div align="center">

<img src="resources/icon.svg" alt="" align="center" height="64" />

# Orchbee

**一个运行和编排 AI Agent 的现代终端——跨桌面端、Web 端和移动端。**

[![License](https://img.shields.io/badge/License-TBD-blue?style=flat-square)]() [![TypeScript](https://img.shields.io/badge/TypeScript-blue?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org) [![Electron](https://img.shields.io/badge/Electron-47848C?style=flat-square&logo=electron&logoColor=white)](https://www.electronjs.org) [![Node.js](https://img.shields.io/badge/Node.js->=22-3c873a?style=flat-square&logo=node.js&logoColor=white)](https://nodejs.org)

[English](README.md) · [中文](README.zh-CN.md)

[核心特性](#核心特性) · [安装](#安装) · [技术栈](#技术栈) · [社区](#社区)

<!-- TODO: 添加主界面截图/GIF，展示分屏面板和 Agent 运行效果 -->

</div>

---

Orchbee 是为 AI Agent 时代打造的终端原生环境。它支持以你熟悉的方式运行 Claude Code、Codex、OpenCode、Pi 等 CLI Agent，并赋予它们一项自身不具备的能力：**协同工作**。

Orchbee 将彼此孤立的终端面板变成管理整个 AI Agent 军团的指挥中心。借助原生互操作 CLI，Agent 可以 prompt 其他 Agent、按需拉起新的 Agent、分派任务并协调工作。

会话持续运行，状态在桌面端、Web 端和移动端之间保持同步。从电脑切换到手机后，你仍可继续控制同一批 Agent，无需重新开始。

名字由 **orch**estrator（编排者）和 **bee**（蜜蜂）组合而来，寓意独立个体协同如一。

> [!NOTE]
> Orchbee 不替代你现有的 CLI Agent，而是提供让它们协同工作的终端环境。它让你的角色从 **human-in-the-loop** 转向 **human-on-the-loop**：不再亲自推动每一步，而是监督整个系统，并在必要时介入。

## 核心特性

### 高性能终端

<!-- TODO: 添加 GIF 展示大量输出（如构建日志、大型 git diff）下的流畅渲染 -->

Orchbee 将 PTY 与 Ghostty VT 模拟的权威实现保留在轻量的原生 Terminal Host 中。Electron 与经过认证的浏览器客户端通过同一套 Web renderer 消费按显示节奏生成的投影帧，并在可用时使用 WebGPU，否则使用后备 renderer。客户端不会再次解析 PTY 输出。

### Agent 间协作

<!-- TODO: 添加 GIF，展示一个 Agent 拉起另一个 Agent、分派任务并接收结果 -->

不再逐个盯着彼此孤立的终端。Agent 可以 prompt 和拉起其他 Agent、读取彼此的输出、跨终端发送命令、分派任务并协同推进；你只需在一个地方掌握全局。

当前实验性的本地自动化基础有意保持更窄的边界：每个由 Desktop 创建的 Terminal 都会获得一个精确的
`ORCHBEE_CLI` path，用于检查 live local coordinator 并调用显式准入的 semantic-command catalog。它不需要
全局安装，也不授予任意 shell、file、focus 或 remote authority。参见
[Orchbee CLI 契约](apps/cli/README.zh-CN.md)。

### 从任意设备控制 Agent

<!-- TODO: 添加 GIF，展示运行中的会话在桌面端、Web 端和移动端之间无中断切换 -->

在桌面端启动 Agent，通过 Web 查看进度，再用手机随时接管。会话持续运行并在设备间保持同步；两台桌面客户端还可直接连接，无需云端中转。

### 集群计算

<!-- TODO: 添加示意图，展示 Primary 节点通过多个 Worker 节点扩展能力 -->

当本机资源不足时，可将额外的机器注册为 Worker 节点，为 Agent 集群扩展算力、存储和运行环境。Worker 作为本机能力的延伸，Agent 无需改变原有使用方式。

### 让 Agent 直接使用浏览器

<!-- TODO: 添加 GIF，展示 Agent 启动浏览器、检查页面并验证 UI 变更 -->

Agent 可以直接从终端启动和控制浏览器、检查页面、截图并验证 UI 变更，无需安装浏览器扩展或手动配置 CDP。

## 安装

> [!IMPORTANT]
> Orchbee 正处于积极开发的早期阶段，暂未提供预编译应用；当前源码主要面向贡献者。

<details>
<summary>从源码构建并验证</summary>

**前置条件：** Node.js >=22、Java >=17、通过 rustup 安装的 Rust 1.97.0、Zig 0.16.0、Corepack、Git

```bash
git clone https://github.com/orchbee/orchbee.git
cd orchbee
corepack enable
pnpm install
pnpm typecheck
pnpm test
pnpm build
```

使用确定性的内存 Session 适配器启动浏览器 Workbench Preview：

```bash
pnpm web
```

如需体验完整的本地多客户端流程，先构建一次原生 Terminal Host，并保持 `pnpm web` 运行。然后在第二个终端中启动 Electron；Electron 会加载同一个 Web 开发 renderer：

```bash
pnpm --filter @orchbee/terminal-host build
pnpm desktop
```

Desktop 会打印一个 `ORCHBEE_BROWSER_CLIENT_URL`。在浏览器中打开这个完整 URL，即可与 Electron 加入同一个共享 Workbench 文档和 Host 终端会话。直接打开未带标记的 `pnpm web` URL 时，仍然是隔离的 Preview。

在 Desktop 创建的 Terminal 内，使用以下命令检查精确的 running local coordinator：

```bash
"$ORCHBEE_CLI" status
```

在 PowerShell 中使用 `& $env:ORCHBEE_CLI status`。Desktop 会注入这个精确 launcher；Orchbee
不会全局安装它，也不会把它加入 `PATH`。

</details>

## 技术栈

- **Electron** — 桌面应用外壳
- **React** — 桌面端与 Web 端界面
- **TypeScript** — 应用与共享包代码
- **Node.js** — 仓库工具和 Electron 特权运行时
- **Rust** — 固定版本的原生应用 workspace 与工具链
- **Zig** — 原生终端 Host 运行时
- **Ghostty VT** — 由原生 Terminal Host 负责的终端模拟
- **WebGPU** — GPU 加速终端渲染

## 社区

- [Discord](https://discord.gg/orchbee) — 产品讨论与社区交流
- [GitHub Issues](https://github.com/orchbee/orchbee/issues) — Bug 报告与功能请求
