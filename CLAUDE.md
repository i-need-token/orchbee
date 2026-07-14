# CLAUDE.md

> Guide for AI agents working on this project. Read this before making any changes.

**What is this project**: Orchbee — a modern terminal that runs and orchestrates AI agents, connected across desktop, web, and mobile.

`CLAUDE.md` is a map, not an encyclopedia. It serves as an entry index (kept under ~80 lines), not a knowledge base. Detailed specs live in `docs/`; agents navigate deeper on demand.

## Hard Rules

- **English is the canonical language.** All code and canonical documentation must be in English; localized documentation is allowed only as a locale-suffixed counterpart to an English source, such as `README.zh-CN.md`.
- **Documentation is part of the source of truth.** Update affected docs in the same change whenever code, configuration, commands, or public behavior changes; resolve implementation-doc conflicts before continuing dependent work.
- **Act on structural smells early.** When an in-scope change reveals a clear risk of growth, coupling, ownership ambiguity, or boundary erosion, resolve it before it spreads; if the remedy would materially expand scope, surface the risk before proceeding.
- **AGENTS.md is a symlink to CLAUDE.md.** Only edit CLAUDE.md.
- **Never modify lint config to escape errors.** Fix the code, not the config. Changing `.oxlintrc.json` requires explicit human approval.

## Key Docs Index

- **Architecture**: `docs/ARCHITECTURE.md` — system architecture, dependency direction, data flow
- **Module Boundaries**: `docs/MODULE-BOUNDARIES.md` — source-tree visibility, entry points, composition, and cycles
- **Packages**: `docs/PACKAGES.md` — package naming, exports, builds, dependencies, and versioning
- **Code Style**: `docs/CODESTYLE.md` — comments, naming, file structure conventions
- **Documentation**: `docs/DOCUMENTATION.md` — read before adding, moving, or restructuring docs or changing documented engineering contracts
- **Linting**: `docs/LINTING.md` — commands, enforcement boundary, and custom-rule ownership
- **TypeScript**: `docs/TYPESCRIPT.md` — zero-compromise type safety, forbidden patterns
- **Testing**: `docs/TESTING.md` — test layout, execution, and regression requirements

## Project Structure

```
apps/           # Application entries (main, web, cli)
packages/       # Shared libraries (core, bridge, etc.)
docs/           # Detailed specs and guides
research/       # Time-bounded investigations; never an authoritative source
prompts/        # Task specifications for AI-driven work
```

## Tech Stack

- **Runtime**: Electron + Node.js (>=22)
- **Language**: TypeScript 7.0 (strict mode, `noUncheckedIndexedAccess`, `erasableSyntaxOnly`, `verbatimModuleSyntax`)
- **Frontend**: React + Web
- **Lint**: oxlint (with typescript, unicorn, oxc, react, import plugins)
- **Format**: oxfmt
- **Schema**: Zod 4 (parse at boundaries, trust inside)
- **Monorepo**: pnpm workspaces (`apps/*`, `packages/*`)
