<div align="center">

<img src="resources/icon.svg" alt="" align="center" height="64" />

# Orchbee

**A terminal for running and coordinating AI agents.**

[![License](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](LICENSE)

[English](README.md) · [中文](README.zh-CN.md)

[What it does](#what-it-does) · [Status](#status) · [Try it from source](#try-it-from-source) · [Documentation](#documentation)

</div>

---

> [!IMPORTANT]
> Orchbee is pre-release. The first public beta, `0.1.0-beta.1` for Windows x64, is being prepared, and there is nothing to download yet. Until then you can [run it from source](#try-it-from-source). macOS and Linux follow the Windows beta.

Orchbee is a terminal built for working with AI agents. You run the agent CLIs you already use, such as Claude Code, Codex, OpenCode and Pi, exactly as before. Orchbee adds what a plain terminal lacks: one place that shows what every agent is doing, and a way for agents to work with each other.

The name joins **orch**estrator and **bee**: independent workers that coordinate as one.

## What it does

- **Every agent at a glance.** The sidebar lists the agents in all your Workspaces with their state: working, waiting for you, or done. You stop cycling through terminals to find the one that needs you.
- **Sessions that stay.** Close the window or detach, and your shells and agents keep running. Start Orchbee again and you are back where you left off.
- **Workspaces, Tabs, Panels and Worktrees.** Keep one Workspace per project folder, split it into Panels, and open a Git worktree as its own Workspace when an agent should work on a separate branch.
- **Agents that work together.** Through the `orchbee` command, an agent can start another agent, send it a prompt, wait until it has finished and read its output. Commands answer in JSON on request, for scripts.
- **Two interfaces, one experience.** A terminal interface that runs inside the terminal you already have, and a desktop window. Both show the same Session with the same words and shortcuts.
- **Local and private.** Orchbee runs on your computer, needs no account and sends no analytics. It leaves an agent's own configuration alone unless you turn an integration on.

Orchbee does not replace your agents. It gives them a shared place to work, so that you supervise the whole instead of driving every step.

## Agents working together

These commands are what an agent, a script or you can run from any terminal inside Orchbee. `review` is the name of an agent, as `orchbee agent list` shows it.

```powershell
orchbee agent start codex --split right
orchbee agent prompt review "Review the current changes"
orchbee agent wait review --until idle --timeout 30000
orchbee agent read review --source recent --lines 40
```

`orchbee --help` lists every command. The [native application guide](apps/orchbee/README.md) describes them in detail.

## Status

| Area                                      | State                             |
| ----------------------------------------- | --------------------------------- |
| Terminal interface and `orchbee` commands | Works on Windows, from source     |
| Desktop window                            | Works on Windows, from source     |
| Installer and npm package                 | In preparation for the first beta |
| Other computers over SSH                  | In progress                       |
| macOS and Linux                           | Planned, after the Windows beta   |
| Web and mobile                            | Later                             |

Interfaces are available in English and Simplified Chinese.

## Try it from source

Windows 11 with [Windows Terminal](https://aka.ms/terminal) is the supported setup today.

**You need:** Git, Node.js 22 or later with Corepack, Rust 1.97.0 through rustup, and Zig 0.16.0.

```powershell
git clone https://github.com/i-need-token/orchbee.git
cd orchbee
corepack enable
pnpm install
```

Open the desktop window. This builds the native programs and starts the window on them:

```powershell
pnpm desktop
```

Or build the terminal interface into a new folder and run the program the command prints:

```powershell
node scripts/stage-local-runtime.mjs --output "$env:LOCALAPPDATA\OrchbeeNative"
```

In the terminal interface the prefix key is `Ctrl+B`. Follow it with `|` or `-` to split, `h` `j` `k` `l` to move between Panels, and `d` to detach. Mouse input works too.

A build from source is a development build: it keeps its data apart from an installed release and never changes your `PATH`.

## Documentation

- [Native application guide](apps/orchbee/README.md): Sessions, commands, keys and configuration
- [Architecture](docs/ARCHITECTURE.md): the programs Orchbee is made of and how they talk
- [Glossary](docs/GLOSSARY.md): what Session, Workspace, Tab, Panel and Agent mean here
- [Development](docs/DEVELOPMENT.md) and [Verification](docs/VERIFICATION.md): how changes are made and checked

Orchbee is written in Rust (terminal interface, commands, Server and Worker), Zig with Ghostty's terminal emulation (Terminal Host), and TypeScript with React and Electron (desktop window).

## Contributing and security

Bug reports and ideas are welcome in [GitHub Issues](https://github.com/i-need-token/orchbee/issues). Please report a vulnerability privately, as the [security policy](SECURITY.md) describes.

## License

[MIT](LICENSE). Third-party components are listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
