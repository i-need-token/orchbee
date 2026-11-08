<div align="center">

<img src="resources/icon.svg" alt="" align="center" height="64" />

# Orchbee

**一个运行和协调 AI Agent 的现代终端。**

[![License](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](LICENSE) [![TypeScript](https://img.shields.io/badge/TypeScript-blue?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org) [![Electron](https://img.shields.io/badge/Electron-47848C?style=flat-square&logo=electron&logoColor=white)](https://www.electronjs.org) [![Node.js](https://img.shields.io/badge/Node.js->=22-3c873a?style=flat-square&logo=node.js&logoColor=white)](https://nodejs.org)

[English](README.md) · [中文](README.zh-CN.md)

[核心特性](#核心特性) · [安装](#安装) · [技术栈](#技术栈) · [社区](#社区)

<!-- TODO: 添加主界面截图/GIF，展示分屏面板和 Agent 运行效果 -->

</div>

---

Orchbee 是为 AI Agent 时代打造的终端原生环境。它支持以你熟悉的方式运行 Claude Code、Codex、OpenCode、Pi 等 CLI Agent，并赋予它们一项自身不具备的能力：**协同工作**。

Orchbee 将彼此孤立的终端面板变成管理 AI Agent 的指挥中心。借助原生 `orchbee` CLI，Agent 可以拉起其他 Agent、prompt 它们、读取它们的输出，并向任意终端发送命令。

名字由 **orch**estrator（编排者）和 **bee**（蜜蜂）组合而来，寓意独立个体协同如一。

> [!NOTE]
> Orchbee 不替代你现有的 CLI Agent，而是提供让它们协同工作的终端环境。它让你的角色从 **human-in-the-loop** 转向 **human-on-the-loop**：不再亲自推动每一步，而是监督整个系统，并在必要时介入。

## 核心特性

### 高性能终端

<!-- TODO: 添加 GIF 展示大量输出（如构建日志、大型 git diff）下的流畅渲染 -->

Orchbee 将 PTY 与 Ghostty VT 模拟的权威保留在轻量原生 Terminal Host 中。原生 CLI/TUI 与保留的本地 Electron 客户端通过各自的原生和 Web 渲染器消费按显示节奏投影的帧。Web 渲染器在可用时使用 WebGPU，并提供后备渲染器。客户端不会重新解析 PTY 输出。

### Agent 间协作

<!-- TODO: 添加 GIF，展示一个 Agent 拉起另一个 Agent、prompt 它并读取结果 -->

不再逐个盯着彼此孤立的终端。Agent 可以拉起和 prompt 其他 Agent、读取其他终端的输出、等待 Agent 结束，并跨终端发送命令；你只需在一个地方掌握全局。

当前开发重点是原生 Rust CLI/TUI 及其本地 Server/Worker/Terminal Host 链路。已退役的 Desktop 内嵌 TypeScript CLI、浏览器接入与 POSIX SSH Environment 链路不再是运行入口。已交付命令与开发边界见 [原生 Orchbee 应用](apps/orchbee/README.md)。

## 安装

> [!IMPORTANT]
> Orchbee 正处于积极开发的早期阶段，暂未提供预编译应用；当前源码主要面向贡献者。Windows 是当前主要平台，macOS 与 Linux 支持暂缓。

<details>
<summary>从源码构建并验证</summary>

**前置条件：** Node.js >=22、Java >=17、通过 rustup 安装的 Rust 1.97.0、Zig 0.16.0、Corepack、Git

```bash
git clone https://github.com/i-need-token/orchbee.git
cd orchbee
corepack enable
pnpm install
pnpm verify
```

日常修改使用[验证规范](docs/VERIFICATION.md)中的 `pnpm verify:changed -- <base>`。`pnpm build` 只准备共享库。完整的本机应用构建使用显式 `pnpm build:full`，其他平台的编译使用 `pnpm test:cross`。

从源码在开发版 Desktop 窗口中运行 GUI。该命令会构建原生 CLI、Server、Worker 和 Terminal Host，启动 Web renderer 的开发服务，并在其上打开 Electron：

```bash
pnpm desktop
```

`pnpm web` 只提供 Electron 加载的 renderer；在浏览器中打开该页面不会渲染任何内容。

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
- [GitHub Issues](https://github.com/i-need-token/orchbee/issues) — Bug 报告与功能请求
