<div align="center">

<img src="resources/icon.svg" alt="" align="center" height="64" />

# Orchbee

**A modern terminal for running and coordinating AI agents.**

[![License](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](LICENSE) [![TypeScript](https://img.shields.io/badge/TypeScript-blue?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org) [![Electron](https://img.shields.io/badge/Electron-47848C?style=flat-square&logo=electron&logoColor=white)](https://www.electronjs.org) [![Node.js](https://img.shields.io/badge/Node.js->=22-3c873a?style=flat-square&logo=node.js&logoColor=white)](https://nodejs.org)

[English](README.md) · [中文](README.zh-CN.md)

[Features](#features) · [Install](#install) · [Tech Stack](#tech-stack) · [Community](#community)

<!-- TODO: Add hero screenshot/GIF showing the main interface with split panels and agents running -->

</div>

---

Orchbee is a terminal-native environment built for the age of AI agents. It lets you run CLI agents such as Claude Code, Codex, OpenCode, and Pi without changing how you already use them, while giving them something they lack on their own: **the ability to work together**.

Orchbee turns isolated terminal panels into a command center for your AI agents. Through the native `orchbee` CLI, agents can start other agents, prompt them, read their output, and send commands to any terminal.

The name combines **orch**estrator with **bee**—independent workers coordinating as one.

> [!NOTE]
> Orchbee does not replace your existing CLI agents; it provides the terminal environment in which they can work together. It shifts your role from **human-in-the-loop** to **human-on-the-loop**: instead of driving every step, you supervise the system and step in when needed.

## Features

### High-Performance Terminal

<!-- TODO: Add GIF showing fast rendering with heavy output (e.g. build logs, large git diffs) -->

Orchbee keeps PTY and Ghostty VT emulation authoritative in the lightweight native Terminal Host. Native CLI/TUI and the retained local Electron client consume the same display-cadenced projected frames through their native and Web renderers. The Web renderer uses WebGPU when available with a fallback renderer. Clients never reparse PTY output.

### Agent-to-Agent Collaboration

<!-- TODO: Add a GIF showing one agent starting another, prompting it, and reading the result -->

Stop watching isolated terminals one at a time. Agents can start and prompt other agents, read another terminal's output, wait until an agent finishes, and send commands across terminals, while you keep the whole picture in one place.

Current development focuses on the native Rust CLI/TUI and its local Server/Worker/Terminal Host chain. The retired Desktop-bundled TypeScript CLI, Browser access, and POSIX SSH Environment chain are no longer runtime entry points. See the [native Orchbee application](apps/orchbee/README.md) for delivered commands and development boundaries.

## Install

> [!IMPORTANT]
> Orchbee is in active early development. Pre-built applications are not yet available; the source checkout is currently intended for contributors. Windows is the primary platform; macOS and Linux support is deferred.

<details>
<summary>Build and verify from source</summary>

**Prerequisites:** Node.js >=22, Java >=17, Rust 1.97.0 through rustup, Zig 0.16.0, Corepack, Git

```bash
git clone https://github.com/i-need-token/orchbee.git
cd orchbee
corepack enable
pnpm install
pnpm verify
```

For daily edits, use `pnpm verify:changed -- <base>` as described in [Verification](docs/VERIFICATION.md). `pnpm build` prepares shared libraries only. Complete host application builds use explicit `pnpm build:full`; foreign-target compilation uses `pnpm test:cross`.

Run the GUI from source in the development Desktop window. The command builds the native CLI, Server, Worker, and Terminal Host, serves the Web renderer, and opens Electron on it:

```bash
pnpm desktop
```

`pnpm web` only serves the renderer that Electron loads; a page opened in a browser renders nothing.

</details>

## Tech Stack

- **Electron** — desktop application shell
- **React** — desktop and web interfaces
- **TypeScript** — application and shared package code
- **Node.js** — repository tooling and Electron privileged runtime
- **Rust** — pinned native application workspace and toolchain
- **Zig** — native terminal Host runtime
- **Ghostty VT** — terminal emulation owned by the native Terminal Host
- **WebGPU** — GPU-accelerated terminal rendering

## Community

- [Discord](https://discord.gg/orchbee) — product discussion and community support
- [GitHub Issues](https://github.com/i-need-token/orchbee/issues) — bug reports and feature requests
