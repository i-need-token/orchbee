# CLAUDE.md

> Guide for AI agents working on this project. Read this before making any changes.

**What is this project**: Orchbee — a modern terminal that runs and orchestrates AI agents, connected across desktop, web, and mobile.

`CLAUDE.md` is a map, not an encyclopedia. It serves as an entry index (kept under ~80 lines), not a knowledge base. Repository standards live in `docs/`; package behavior and maintenance references stay with their owning package.

## Hard Rules

- **English is canonical; every repository Markdown document is bilingual.** English uses the unsuffixed `.md` filename. Pair standards under `docs/` through the same path under `docs/zh-CN/`; pair every other project-owned Markdown file with a same-directory `.zh-CN.md` counterpart. English resolves conflicts, and runtime configuration, diagnostics, and engineering authority use English sources.
- **Bilingual documentation never drifts.** Every Markdown addition, deletion, move, structural edit, example, link, or content change must update both languages in the same change. After semantic review, confirm every affected pair with `pnpm docs:confirm-pairing -- <markdown-path>`; the required lint check rejects missing, structurally different, or unconfirmed pairs.
- **Product copy is localized.** Every product-facing message goes through the localization system and ships in both `en-US` and `zh-CN`; locale resources are product assets and may contain their target language.
- **Documentation authority follows ownership.** Update the owning repository standard, package README, package-local reference, proposal, or research record in the same change as the fact it governs. Repository standards never inventory one package's business behavior, source tree, screen composition, or roadmap; future design belongs under `proposals/`.
- **Every tracked file must have durable repository value.** Before adding a file, identify its future consumer, authoritative responsibility, recurring use, and maintenance lifecycle; task history and one-off working artifacts do not belong in the repository.
- **Classify changes before governed edits.** Follow `docs/DEVELOPMENT.md`. Behavioral, structural, and consequential work needs accepted change authorization before editing governed artifacts; new reserved decisions return to the owning stage instead of being silently encoded.
- **Act on structural smells early.** When an in-scope change reveals a clear risk of growth, coupling, ownership ambiguity, or boundary erosion, resolve it before it spreads; if the remedy would materially expand scope, surface the risk before proceeding.
- **`AGENTS.md` and `AGENTS.zh-CN.md` are symlinks to their `CLAUDE` counterparts.** Only edit `CLAUDE.md` and `CLAUDE.zh-CN.md`.
- **Never modify lint config to escape errors.** Fix the code, not the config. Changing `.oxlintrc.json` requires explicit human approval.
- **Desktop development diagnostics target the development application identity.** When inspecting `pnpm desktop`, target the generated `Orchbee Dev.app` by its exact path or checkout-scoped bundle identifier. Never open, focus, quit, replace, or fall back to `/Applications/Orchbee.app` unless the human explicitly requests the installed release application in the current task.
- **The focus-stealing Desktop native-window system test is manual-only.** Never invoke `test:system:manual:native-window` or set `ORCHBEE_DESKTOP_NATIVE_WINDOW_TEST_APPROVAL` unless the human explicitly authorizes that exact test in the current request; aggregate, full, CI, and autonomous verification must exclude it. See `apps/desktop/README.md#manual-native-window-test`.

## Key Docs Index

- **Architecture**: `docs/ARCHITECTURE.md` — system architecture, dependency direction, data flow
- **Applications**: `docs/APPLICATIONS.md` — runnable workspace placement, shape, and lifecycle
- **Module Boundaries**: `docs/MODULE-BOUNDARIES.md` — semantic module design, source-tree visibility, entry points, composition, and cycles
- **Packages**: `docs/PACKAGES.md` — package naming, exports, builds, dependencies, and versioning
- **Projects**: `docs/PROJECTS.md` — machine-readable workspace scope and responsibility contracts
- **Code Style**: `docs/CODESTYLE.md` — comments, naming, file structure conventions
- **Documentation**: `docs/DOCUMENTATION.md` — read before adding, moving, or restructuring docs or changing documented engineering contracts
- **Repository Hygiene**: `docs/REPOSITORY-HYGIENE.md` — tracked-file admission, temporary artifacts, generated files, and pre-handoff review
- **Linting**: `docs/LINTING.md` — commands, enforcement boundary, and custom-rule ownership
- **Verification**: `docs/VERIFICATION.md` — the canonical local and CI quality gate
- **Development**: `docs/DEVELOPMENT.md` — change classification, decision authority, staged governed-write control, and convergence
- **Modeling and Correctness**: `docs/MODELING-AND-CORRECTNESS.md` — model-first behavior design and risk-based correctness evidence
- **TypeScript**: `docs/TYPESCRIPT.md` — zero-compromise type safety, forbidden patterns
- **Rust**: `docs/RUST.md` — Cargo workspace placement, ownership, safety, concurrency, contracts, and verification
- **Zig**: `docs/ZIG.md` — native naming, resource ownership, safety, concurrency, and testing
- **Testing**: `docs/TESTING.md` — test layout, execution, and regression requirements
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

## Project Structure

```
apps/           # Independently runnable product and development applications
crates/         # Reusable Rust libraries admitted for real cross-crate consumers
packages/       # Shared JavaScript and TypeScript libraries, guides, and local references
docs/           # Repository-wide standards and cross-workspace architecture contracts
proposals/      # Future design under review; never current authority
research/       # Time-bounded investigations; never an authoritative source
prompts/        # Task specifications for AI-driven work
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
