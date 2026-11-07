# CLAUDE.md

> Guide for AI agents working on this project. Read this before making any changes.

**What is this project**: Orchbee — a modern terminal that runs and orchestrates AI agents, connected across desktop, web, and mobile.

`CLAUDE.md` is a map, not an encyclopedia. It serves as an entry index (kept under ~80 lines), not a knowledge base. Repository standards live in `docs/`; package behavior and maintenance references stay with their owning package.

## Hard Rules

- **Repository Markdown is English.** Use unsuffixed `.md` filenames and do not add Chinese translations. The only exceptions are `README.zh-CN.md`, updated with `README.md` in the same change, and Markdown embedded in a shipped binary, which is product copy. English resolves conflicts, and runtime configuration, diagnostics, and engineering authority use English sources.
- **Product copy is localized.** Every product-facing message goes through the localization system and ships in both `en-US` and `zh-CN`; locale resources are product assets and may contain their target language.
- **Documentation authority follows ownership.** Update the owning repository standard, package README, package-local reference, proposal, or research record in the same change as the fact it governs. Repository standards never inventory one package's business behavior, source tree, screen composition, or roadmap; future design belongs under `proposals/`.
- **Every tracked file must have durable repository value.** Before adding a file, identify its future consumer, authoritative responsibility, recurring use, and maintenance lifecycle; task history and one-off working artifacts do not belong in the repository.
- **Complete authorized work autonomously.** Follow `docs/DEVELOPMENT.md`: classify risk before editing, treat a clear request as authorization for its complete scoped result, and continue through implementation, checks, and ordinary repairs. Escalate new reserved decisions; pause only their dependent work.
- **Routine verification does not require full builds.** Use the affected host gate in `docs/VERIFICATION.md` for ordinary edits and merges. Full application builds and cross-compilation use explicit commands when the changed build, packaging, or platform contract needs them; do not repeat a gate merely to merge unchanged checked inputs.
- **Do not add unit tests by default.** Rust, Zig, and TypeScript use the admission and removal rule in `docs/TESTING.md`: justify a meaningful evidence gap before adding tests, assert rules rather than one design, delete a design-pinning test that an intended change breaks instead of updating its expectations, and remove low-value companions while retaining necessary behavior and regression evidence.
- **Act on structural smells early.** When an in-scope change reveals a clear risk of growth, coupling, ownership ambiguity, or boundary erosion, resolve it before it spreads; if the remedy would materially expand scope, surface the risk before proceeding.
- **`AGENTS.md` is a symlink to `CLAUDE.md`.** Only edit `CLAUDE.md`.
- **Never modify lint config to escape errors.** Fix the code, not the config. Changing `.oxlintrc.json` requires explicit human approval.
- **Desktop development diagnostics target the development application identity.** When inspecting `pnpm desktop`, target the generated `Orchbee Dev.app` by its exact path or checkout-scoped bundle identifier. Never open, focus, quit, replace, or fall back to `/Applications/Orchbee.app` unless the human explicitly requests the installed release application in the current task.
- **Process termination is identity-checked.** Agents never run raw termination commands (`taskkill`, `Stop-Process`, `kill`, `Process.Kill`); the PreToolUse guard refuses them. Stop only a process this task started: run `node scripts/process-guard.mjs check --pid <PID> --started <UTC-START>`, then `stop` with the same arguments. Agents never stop the installed production Orchbee under `%LOCALAPPDATA%\Programs\Orchbee`; ask the human.

## Key Docs Index

- **Architecture**: `docs/ARCHITECTURE.md` — system architecture, dependency direction, data flow
- **Glossary**: `docs/GLOSSARY.md` — product concepts, canonical names, identity boundaries, and localized terminology
- **Cross-client Interface Design**: `docs/INTERFACE-DESIGN.md` — shared GUI/TUI semantics, shortcut transfer, visual roles, interaction patterns, and paired delivery
- **Applications**: `docs/APPLICATIONS.md` — runnable workspace placement, shape, and lifecycle
- **Module Boundaries**: `docs/MODULE-BOUNDARIES.md` — semantic module design, source-tree visibility, entry points, composition, and cycles
- **Packages**: `docs/PACKAGES.md` — package naming, exports, builds, dependencies, and versioning
- **Projects**: `docs/PROJECTS.md` — machine-readable workspace scope and responsibility contracts
- **Code Style**: `docs/CODESTYLE.md` — comments, naming, file structure conventions
- **Documentation**: `docs/DOCUMENTATION.md` — read before adding, moving, or restructuring docs or changing documented engineering contracts
- **Repository Hygiene**: `docs/REPOSITORY-HYGIENE.md` — tracked-file admission, temporary artifacts, generated files, and pre-handoff review
- **Linting**: `docs/LINTING.md` — commands, enforcement boundary, and custom-rule ownership
- **Verification**: `docs/VERIFICATION.md` — the canonical local and CI quality gate
- **Development**: `docs/DEVELOPMENT.md` — scoped authorization, risk classification, continuous delivery, and convergence
- **Modeling and Correctness**: `docs/MODELING-AND-CORRECTNESS.md` — model-first behavior design and risk-based correctness evidence
- **TypeScript**: `docs/TYPESCRIPT.md` — zero-compromise type safety, forbidden patterns
- **Rust**: `docs/RUST.md` — Cargo workspace placement, crate shape, dependency admission, platform gating, ownership, safety, concurrency, process boundaries, localization, contracts, and verification
- **Zig**: `docs/ZIG.md` — native naming, resource ownership, safety, concurrency, and testing
- **Testing**: `docs/TESTING.md` — test admission, removal, layout, execution, and regression evidence
- **Frontend Package Boundaries**: `docs/frontend/package-boundaries.md` — frontend workspace placement, semantic ownership, dependency direction, and platform ports
- **Frontend React**: `docs/frontend/react.md` — component ownership, file boundaries, state, Effects, Hooks, Zustand, and testing
- **Frontend Experience Components**: `docs/frontend/experience-components.md` — reviewed interaction vocabulary, package-local manifests, discovery, composition constraints, and lifecycle
- **Frontend Dialogs and Forms**: `docs/frontend/dialogs-and-forms.md` — interruption budgets, surface admission, question budgets, cancellation, focus, and evidence
- **Frontend Settings**: `docs/frontend/settings.md` — setting admission, visual composition, scope, controls, commit, reset, and evidence
- **Frontend Visual Foundations**: `docs/frontend/visual-foundations.md` — typography, hierarchy, spacing, radius, sizing, border, and surface selection rules
- **Frontend Design Tokens**: `docs/frontend/design-tokens.md` — token ownership, tiers, naming, themes, semantic coverage, and changes
- **Frontend Styling**: `docs/frontend/styling.md` — CSS Modules, generated shadcn boundaries, icons, stylesheets, and verification
- **Frontend Storybook**: `docs/frontend/storybook.md` — host ownership, Story placement, navigation, authoring, and browser verification
- **Frontend Localization**: `docs/frontend/localization.md` — locale ownership, message catalogs, formatting, fallback, and verification
- **Frontend Menus**: `docs/frontend/menus.md` — menu triggers, geometry, content, interaction states, keyboard behavior, and verification
- **Frontend Tooltips**: `docs/frontend/tooltips.md` — provider ownership, delay, content, accessibility, and verification
- **Frontend Actions**: `docs/frontend/actions.md` — semantic actions, registration, availability, synchronous and asynchronous execution, cancellation, concurrency, and verification
- **Frontend Keyboard Shortcuts**: `docs/frontend/keyboard-shortcuts.md` — action ownership, bindings, platform display, conflicts, and verification
- **Frontend Browser Storage**: `docs/frontend/browser-storage.md` — local persistence ownership, key versioning, validation, privacy, and verification
- **TUI Visual Foundations**: `docs/tui/visual-foundations.md` — color roles, theme contract, text hierarchy, frames, glyphs, and display width
- **TUI Components**: `docs/tui/components.md` — component ownership, reuse, control states, action affordance, lists, editors, pickers, and shared chrome
- **TUI Actions**: `docs/tui/actions.md` — semantic identity, availability, invocation, execution, concurrency, cancellation, and outcomes
- **TUI Settings**: `docs/tui/settings.md` — setting metadata, scope, value states, preview, commit, reset, and persistence feedback
- **TUI Layout**: `docs/tui/layout.md` — regions, size tiers, degradation order, overlay geometry, truncation, and resize
- **TUI Keyboard Input**: `docs/tui/keyboard-input.md` — key decoding, binding model, input ownership, editing conventions, and discovery
- **TUI Mouse Input**: `docs/tui/mouse-input.md` — capture policy, hit testing, gesture vocabulary, passthrough, wheel normalization, and selection
- **TUI Overlays and Status**: `docs/tui/overlays-and-status.md` — surface selection, overlay composition, confirmations, status line, notices, and pending indication
- **TUI Terminal Environment**: `docs/tui/terminal-environment.md` — host admission, mode lifecycle, capability detection, output discipline, and restoration
- **TUI Verification**: `docs/tui/verification.md` — structural prerequisites and render, input, environment, and host evidence for terminal interfaces
- **CLI Command Surface**: `docs/cli/command-surface.md` — grammar authority, command paths, options, targets, help, completion, destructive commands, Agent discoverability, and environment variables
- **CLI Output**: `docs/cli/output.md` — stream split, formats, machine documents, exit codes, text rendering, bounds, streamed records, and localization
- **CLI Process**: `docs/cli/process.md` — input trust boundary, parse before effect, deadlines, side effects, request identity, signals, and runtime discovery
- **CLI Verification**: `docs/cli/verification.md` — structural prerequisites and grammar, effect, exit-code, text, bounds, and process evidence for command-line surfaces

## Project Structure

```
apps/           # Independently runnable product and development applications
crates/         # Reusable Rust libraries admitted for real cross-crate consumers
packages/       # Shared JavaScript and TypeScript libraries, guides, and local references
docs/           # Repository-wide standards and cross-workspace architecture contracts
proposals/      # Future design under review; never current authority
schemas/        # Machine-readable repository contract schemas
```

## Tech Stack

- **Frontend**: React + TypeScript 7.0 (strict mode, `noUncheckedIndexedAccess`, `erasableSyntaxOnly`, `verbatimModuleSyntax`)
- **Routing**: TanStack Router
- **Client State**: Zustand
- **UI**: shadcn/ui with Base UI primitives
- **Native Toolchains**: Rust 1.97.0 + Zig 0.16.0 with pinned Ghostty VT
- **Tooling Runtime**: Node.js (>=22)
- **Lint**: oxlint (with typescript, unicorn, oxc, react, import plugins)
- **Format**: oxfmt + rustfmt + `zig fmt`
- **Schema**: Zod 4 (parse at boundaries, trust inside)
- **Monorepo**: pnpm workspaces (`apps/*`, `packages/*`) + one root Cargo workspace (`apps/*`, `crates/*` Rust members)
