<div align="center">

<img src="resources/icon.svg" alt="" align="center" height="64" />

# Orchbee

**A modern terminal for running and orchestrating AI agents — across desktop, web, and mobile.**

[![License](https://img.shields.io/badge/License-TBD-blue?style=flat-square)]() [![TypeScript](https://img.shields.io/badge/TypeScript-blue?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org) [![Electron](https://img.shields.io/badge/Electron-47848C?style=flat-square&logo=electron&logoColor=white)](https://www.electronjs.org) [![Node.js](https://img.shields.io/badge/Node.js->=22-3c873a?style=flat-square&logo=node.js&logoColor=white)](https://nodejs.org)

[English](README.md) · [中文](README.zh-CN.md)

[Features](#features) · [Install](#install) · [Tech Stack](#tech-stack) · [Community](#community)

<!-- TODO: Add hero screenshot/GIF showing the main interface with split panels and agents running -->

</div>

---

Orchbee is a terminal-native environment built for the age of AI agents. It lets you run CLI agents such as Claude Code, Codex, OpenCode, and Pi without changing how you already use them, while giving them something they lack on their own: **the ability to work together**.

Orchbee turns isolated terminal panes into a command center for an entire fleet of AI agents. Through a native interop CLI, agents can prompt other agents, spawn new ones on demand, delegate tasks, and coordinate their work.

Sessions remain active, with state synchronized across desktop, web, and mobile. Switch from your desktop to your phone and continue controlling the same agents without starting over.

The name combines **orch**estrator with **bee**—independent workers coordinating as one.

> [!NOTE]
> Orchbee does not replace your existing CLI agents; it provides the terminal environment in which they can work together. It shifts your role from **human-in-the-loop** to **human-on-the-loop**: instead of driving every step, you supervise the system and step in when needed.

## Features

### High-Performance Terminal

<!-- TODO: Add GIF showing fast rendering with heavy output (e.g. build logs, large git diffs) -->

Orchbee keeps PTY and Ghostty VT emulation authoritative in the lightweight native Terminal Host. Electron and authenticated Browser clients consume the same display-cadenced projected frames through the shared Web renderer, using WebGPU when available with a fallback renderer. Clients never reparse PTY output.

### Agent-to-Agent Collaboration

<!-- TODO: Add a GIF showing one agent spawning another, delegating a task, and receiving the result -->

Stop managing agents one terminal at a time. Agents can prompt and spawn other agents, read each other's output, send commands across terminals, delegate tasks, and coordinate work—while you stay in control from one place.

The current experimental local automation foundation is intentionally narrower: every
Desktop-created Terminal receives an exact `ORCHBEE_CLI` path for inspecting the live local
coordinator and invoking the explicitly admitted semantic-command catalog. It requires no global
installation and grants no arbitrary shell, file, focus, or remote authority. See the
[Orchbee CLI contract](apps/cli/README.md).

### Control Agents from Any Device

<!-- TODO: Add a GIF showing a running session moving from desktop to web to mobile without interruption -->

Start agents on your desktop, monitor them from the web, and take control from your phone. Sessions keep running and stay synchronized across devices, and two desktops can connect directly without a cloud relay.

### Cluster Computing

<!-- TODO: Add a visual showing a Primary node extended by multiple Worker nodes -->

When local resources are not enough, register additional machines as Worker nodes to extend the compute, storage, and execution environments available to your agent cluster. Workers act as an extension of the local machine, while agents use them without changing their workflow.

### Give Agents a Browser

<!-- TODO: Add a GIF showing an agent launching a browser, inspecting a page, and verifying a UI change -->

Agents can launch and control browsers directly from the terminal, inspect pages, take screenshots, and verify UI changes—without browser extensions or manual CDP setup.

## Install

> [!IMPORTANT]
> Orchbee is in active early development. Pre-built applications are not yet available; the source checkout is currently intended for contributors.

<details>
<summary>Build and verify from source</summary>

**Prerequisites:** Node.js >=22, Java >=17, Rust 1.97.0 through rustup, Zig 0.16.0, Corepack, Git

```bash
git clone https://github.com/orchbee/orchbee.git
cd orchbee
corepack enable
pnpm install
pnpm typecheck
pnpm test
pnpm build
```

Start the browser Workbench Preview with its deterministic in-memory session adapter:

```bash
pnpm web
```

For the complete local multi-client flow, build the native Terminal Host once and keep `pnpm web` running. In a second terminal, start Electron; it loads that same Web development renderer:

```bash
pnpm --filter @orchbee/terminal-host build
pnpm desktop
```

Desktop prints an `ORCHBEE_BROWSER_CLIENT_URL`. Open that complete URL in a browser to join the same shared Workbench document and Host terminal sessions as Electron. Opening the unmarked `pnpm web` URL directly remains an isolated Preview.

Inside a Terminal created by Desktop, inspect the exact running local coordinator with:

```bash
"$ORCHBEE_CLI" status
```

In PowerShell, use `& $env:ORCHBEE_CLI status`. Desktop injects this exact launcher; Orchbee does
not install it globally or add it to `PATH`.

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
- [GitHub Issues](https://github.com/orchbee/orchbee/issues) — bug reports and feature requests
