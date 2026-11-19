<div align="center">

<img src="resources/icon.svg" alt="" align="center" height="64" />

# Orchbee

**A modern, multiplexed terminal for running and orchestrating AI agents.**

Sessions persist in the background and remain accessible from the terminal, the desktop, and the command line simultaneously, giving people and agents a shared environment to work in.

[![License](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](LICENSE) [![Rust](https://img.shields.io/badge/Rust-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org) [![Zig](https://img.shields.io/badge/Zig-F7A41D?style=flat-square&logo=zig&logoColor=white)](https://ziglang.org) [![TypeScript](https://img.shields.io/badge/TypeScript-blue?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org) [![Electron](https://img.shields.io/badge/Electron-47848C?style=flat-square&logo=electron&logoColor=white)](https://www.electronjs.org)

[English](README.md) · [中文](README.zh-CN.md)

[Features](#features) · [Install](#install) · [Tech Stack](#tech-stack) · [Community](#community)

<img src="resources/readme/hero.png" alt="The Orchbee terminal interface: a sidebar of Workspaces and agents, a shell, and two agents running in split Panels" width="880" />

</div>

---

Orchbee is a modern terminal built for the age of AI agents. It is a complete terminal first, built on the core of [Ghostty](https://ghostty.org), and everything else is built around it.

**Its core is multiplexing.** A conventional terminal session ends when its window closes. An Orchbee Session persists in the background and can be attached from several clients at once: the terminal interface, the desktop application, and the command line present and control the same Session in real time.

**This makes Orchbee a shared environment for people and agents.** Every operation available to a person is equally available to an agent through the `orchbee` command line. CLI agents such as Claude Code, Codex, OpenCode, and Pi run unchanged and gain a capability they lack on their own: **the ability to work together**.

Orchbee recognizes more than 30 agent CLIs, presents the status of each at a glance, and organizes work by project in the sidebar.

The name combines **orch**estrator with **bee**—independent workers coordinating as one.

> [!NOTE]
> Orchbee does not replace your existing CLI agents; it provides the terminal environment in which they can work together. It shifts your role from **human-in-the-loop** to **human-on-the-loop**: instead of driving every step, you supervise the system and step in when needed.

## Features

### High-Performance Terminal

<!-- TODO: Add GIF showing fast rendering with heavy output (e.g. build logs, large git diffs) -->

Terminal emulation runs in a lightweight native process built on the core of Ghostty. The terminal interface and the desktop application receive the finished screen from that process and only draw it. The desktop application renders with WebGPU, with a Canvas 2D fallback.

### Multiplexed Sessions

![The terminal interface and the desktop application attached to the same Session; a command typed in one appears in the other](resources/readme/multiplex.gif)

A Session keeps running in the background after its window is closed, along with the shells and agents in it. The terminal interface, the desktop application, and the command line can attach to the same Session at the same time, and input from one is visible in the others.

### Agent-to-Agent Collaboration

![An agent started, prompted, awaited, and read with orchbee commands from a shell Panel](resources/readme/agents-cli.gif)

Stop managing agents one terminal at a time. Agents can prompt and spawn other agents, read each other's output, send commands across terminals, delegate tasks, and coordinate work—while you stay in control from one place.

Each step is one `orchbee` command:

```bash
orchbee agent start codex --split right
orchbee agent list
orchbee agent prompt @panel:0x36 "Review the current changes"
orchbee agent wait @panel:0x36 --until idle --timeout 120000
orchbee agent read @panel:0x36 --source recent --lines 40
```

`orchbee agent list` prints the handle of each agent, such as `@panel:0x36`.

### Agent Status and Organization

![The Orchbee desktop application: the sidebar lists agents by project and Workspace with their states](resources/readme/desktop.png)

Orchbee recognizes more than 30 agent CLIs without any setup and shows the state of each one: working, needs attention, or done. The sidebar lists every agent across all projects and organizes work by project, Workspace, and Git worktree.

### Remote Machines

<!-- TODO: Add a visual showing the local machine connected to several remote machines -->

When local resources are not enough, other computers can be added over SSH to extend the compute and execution environments available to agents. Remote machines act as an extension of the local machine, and agents use them without changing their workflow. This feature is in development and is planned for the first release.

## Install

> [!IMPORTANT]
> Orchbee is in beta. It runs on Windows (x86_64) and on macOS 13 or later (Apple silicon and
> Intel); Linux builds are in progress.

### Windows: desktop app, terminal and CLI

Download `Orchbee-<version>-windows-x86_64-setup.exe` from the
[releases page](https://github.com/i-need-token/orchbee/releases) and run it.

- Installs for the current user only; no administrator rights needed.
- Adds the `orchbee` command to your `Path`. Open a new terminal to use it.
- The installer is not code-signed yet, so Windows SmartScreen may ask you to confirm.

### macOS: desktop app, terminal and CLI

Install from a terminal:

```sh
curl -fsSL https://orchbee.com/install.sh | sh                  # terminal interface and CLI
curl -fsSL https://orchbee.com/install.sh | sh -s -- --desktop  # the desktop app as well
```

- The desktop app goes into `/Applications` when you can write to it, otherwise into
  `~/Applications`.
- The command is linked at `~/.local/bin/orchbee`, and the installer puts that folder on `PATH` for
  new terminals: two lines in `~/.zshenv` for zsh, or in bash's login file. Open a new terminal to
  use `orchbee`; `orchbee uninstall` removes those lines. For another shell it prints the line to
  add.

Or download the disk image from the
[releases page](https://github.com/i-need-token/orchbee/releases):
`Orchbee-<version>-macos-aarch64.dmg` for Apple silicon, `Orchbee-<version>-macos-x86_64.dmg` for
Intel. Open it and drag **Orchbee** to Applications. Its first start installs the command the same way.

- The app is not notarized by Apple yet, so macOS blocks the first open of a browser download. Click
  **Done**, then **Open Anyway** in **System Settings › Privacy & Security**; the
  [website](https://orchbee.com/#install) has the steps. The terminal install skips this.
- Terminals inside Orchbee find `orchbee`; for other terminals, run the first command above.

Known limits on macOS:

- Intel Macs are tested in CI only.
- Remembered SSH passwords: macOS asks again for Keychain access after each update, until Orchbee is
  signed.
- Images show in terminals that speak the Kitty graphics protocol, such as Ghostty and kitty; iTerm2
  and Terminal.app show a coarser cell picture.

### Terminal and CLI only

```sh
npm install -g orchbee@next
```

Requires Node.js 22 or later. This installs the terminal interface and the CLI, without the desktop app.

### Start

```sh
orchbee          # open the terminal interface
```

Or open **Orchbee** from the Start menu on Windows, or from Applications on macOS.

### Update

```sh
orchbee update
```

Installs the newest version beside the one you have. Running terminals and Agents are not
interrupted: they keep the old version until they are restarted. Open Orchbee again to use the new
one. The desktop app also offers **Update** when a newer version is ready. Version 0.1.0-beta.1 has
no update command, so from it install the next version once by hand, with the installer or npm as
above.

### Uninstall

On Windows, remove Orchbee from **Settings › Apps**, or run `orchbee uninstall`. On macOS, run
`orchbee uninstall`, then move **Orchbee** from Applications to the Trash. For an npm install, run
`orchbee uninstall` and then `npm rm -g orchbee`. Your data in `~/.orchbee` is kept; add
`--delete-data` to remove it too.

<details>
<summary>Build from source</summary>

**Prerequisites:** Windows 11, Git, Node.js >=22, Corepack, Rust 1.97.0 through rustup, Zig 0.16.0

```bash
git clone https://github.com/i-need-token/orchbee.git
cd orchbee
corepack enable
pnpm install
```

Start the desktop application:

```bash
pnpm desktop
```

Build the terminal interface into a folder, then run the program that the command prints:

```powershell
node scripts/stage-local-runtime.mjs --output "$env:LOCALAPPDATA\OrchbeeNative"
```

</details>

## Tech Stack

- **Rust** — terminal interface, command line, and background services
- **Zig** — native terminal process
- **Ghostty VT** — terminal emulation
- **Electron** — desktop application shell
- **React** — desktop interface
- **TypeScript** — desktop application and shared package code
- **WebGPU** — GPU-accelerated terminal rendering
- **Node.js** — repository tooling

## Community

- [GitHub Issues](https://github.com/i-need-token/orchbee/issues) — bug reports and feature requests
- [Security Policy](SECURITY.md) — private vulnerability reporting

## License

[MIT](LICENSE). Third-party components are listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
