<div align="center">

<img src="resources/icon.svg" alt="" align="center" height="64" />

# Orchbee

**一个用来运行和协调 AI 智能体的终端。**

[![License](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](LICENSE)

[English](README.md) · [中文](README.zh-CN.md)

[它能做什么](#它能做什么) · [当前状态](#当前状态) · [从源码试用](#从源码试用) · [文档](#文档)

</div>

---

> [!IMPORTANT]
> Orchbee 尚未正式发布。首个公开测试版 `0.1.0-beta.1`（Windows x64）正在准备中，目前还没有可下载的安装包。在此之前可以[从源码运行](#从源码试用)。macOS 和 Linux 排在 Windows 测试版之后。

Orchbee 是为使用 AI 智能体（Agent）而做的终端。Claude Code、Codex、OpenCode、Pi 这些你已经在用的命令行智能体，照原样运行即可。Orchbee 补上普通终端没有的两样东西：在一个地方看清每个智能体在做什么，以及让智能体之间可以互相配合。

名字由 **orch**estrator（编排者）和 **bee**（蜜蜂）组成：各自独立的个体，协同如一。

## 它能做什么

- **所有智能体一目了然。** 边栏列出所有工作区里的智能体和它们的状态：工作中、等你处理、已完成。不用再逐个终端翻找是哪个在等你。
- **会话一直都在。** 关掉窗口或分离之后，Shell 和智能体继续运行。再次启动 Orchbee，就回到离开时的位置。
- **工作区、标签页、面板和工作树。** 一个项目文件夹一个工作区，可以分屏成多个面板；需要让智能体在单独的分支上工作时，把一个 Git 工作树作为独立的工作区打开。
- **智能体互相配合。** 通过 `orchbee` 命令，一个智能体可以启动另一个智能体、给它发提示词、等它完成、读取它的输出。命令可以按需以 JSON 输出，供脚本使用。
- **两种界面，同一种体验。** 一个运行在你现有终端里的终端界面，和一个桌面窗口。两者显示同一个会话，用词和快捷键一致。
- **本地运行，数据不外传。** Orchbee 运行在你自己的电脑上，不需要账号，不发送任何统计数据。除非你主动开启某项集成，否则不改动智能体自己的配置。

Orchbee 不取代你的智能体。它给它们一个共同工作的地方，让你从逐步操作变为总览全局、必要时介入。

## 智能体互相配合

下面的命令，智能体、脚本或你自己都可以在 Orchbee 里的任何终端中运行。`review` 是一个智能体的名字，与 `orchbee agent list` 显示的一致。

```powershell
orchbee agent start codex --split right
orchbee agent prompt review "Review the current changes"
orchbee agent wait review --until idle --timeout 30000
orchbee agent read review --source recent --lines 40
```

`orchbee --help` 列出全部命令。[原生应用指南](apps/orchbee/README.md)（英文）有详细说明。

## 当前状态

| 部分                      | 状态                          |
| ------------------------- | ----------------------------- |
| 终端界面和 `orchbee` 命令 | 可在 Windows 上从源码运行     |
| 桌面窗口                  | 可在 Windows 上从源码运行     |
| 安装包和 npm 包           | 正在为首个测试版准备          |
| 通过 SSH 使用其他电脑     | 开发中                        |
| macOS 和 Linux            | 计划中，在 Windows 测试版之后 |
| 网页版和移动端            | 以后                          |

界面提供英文和简体中文。

## 从源码试用

目前支持的环境是 Windows 11 加 [Windows Terminal](https://aka.ms/terminal)。

**需要：** Git、Node.js 22 或更高版本（含 Corepack）、通过 rustup 安装的 Rust 1.97.0、Zig 0.16.0。

```powershell
git clone https://github.com/i-need-token/orchbee.git
cd orchbee
corepack enable
pnpm install
```

打开桌面窗口。这条命令会构建原生程序，并在它们之上启动窗口：

```powershell
pnpm desktop
```

或者把终端界面构建到一个新文件夹，然后运行命令打印出的那个程序：

```powershell
node scripts/stage-local-runtime.mjs --output "$env:LOCALAPPDATA\OrchbeeNative"
```

在终端界面里，前缀键是 `Ctrl+B`。之后按 `|` 或 `-` 分屏，按 `h` `j` `k` `l` 在面板间移动，按 `d` 分离。也可以用鼠标操作。

从源码构建的是开发版：它的数据与已安装的正式版分开存放，也不会改动你的 `PATH`。

## 文档

以下文档为英文。

- [原生应用指南](apps/orchbee/README.md)：会话、命令、按键和配置
- [架构](docs/ARCHITECTURE.md)：Orchbee 由哪些程序组成，它们如何通信
- [术语表](docs/GLOSSARY.md)：会话、工作区、标签页、面板、智能体在这里各指什么
- [开发](docs/DEVELOPMENT.md)和[验证](docs/VERIFICATION.md)：改动如何进行、如何检查

Orchbee 使用 Rust（终端界面、命令、Server 和 Worker）、Zig 加 Ghostty 的终端仿真（Terminal Host），以及 TypeScript 加 React 和 Electron（桌面窗口）编写。

## 参与和安全

欢迎在 [GitHub Issues](https://github.com/i-need-token/orchbee/issues) 提交问题和想法。发现安全漏洞请按[安全策略](SECURITY.md)私下报告。

## 许可

[MIT](LICENSE)。第三方组件见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
