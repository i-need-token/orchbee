# Applications

## Placement

| Artifact or decision            | Contract                                                                                                                                                                                      |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Runnable or deployable entry    | Put it in `apps/*`; both end-user products and development applications use this location.                                                                                                    |
| Imported reusable capability    | Put it in `packages/*`.                                                                                                                                                                       |
| Repository automation           | Put it in `scripts/*` when it has no independent application lifecycle.                                                                                                                       |
| Product/development distinction | Keep one `apps/*` root; express the distinction through the workspace name, `private`, `project.json` responsibility and non-goals, and release configuration when a release pipeline exists. |
| Product application name        | Name its user-facing surface or runtime role, not an internal process such as `main` or `renderer`.                                                                                           |
| Headless long-running entry     | Create it only for a concrete lifecycle and consumer; an architecture diagram is insufficient.                                                                                                |

## Required application shape

Every direct application workspace contains:

| Path                            | Responsibility                                                              |
| ------------------------------- | --------------------------------------------------------------------------- |
| `package.json`                  | Identity, commands, and dependencies                                        |
| `project.json`                  | Runtime, stability, responsibility, and non-goals                           |
| `tsconfig.json`                 | Type-checking contract when the application contains TypeScript source      |
| `build.zig` and `build.zig.zon` | Build graph and content-addressed dependency contract for a Zig application |

| Application                 | Script contract                                                                                                                            |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Every application           | Expose the scripts required by its lifecycle.                                                                                              |
| Browser application         | Normally expose `dev`, `build`, `typecheck`, `test`, and `test:watch`.                                                                     |
| Native Zig application      | Expose non-interactive `build`, `format`, `format:check`, and `test`; expose `test:cross` when claiming multiple operating-system targets. |
| Specialized development app | May add explicit workflow commands; keep `build` non-interactive so `pnpm verify` can run it.                                              |

## Workspace development

| Concern            | Contract                                                                                                                                                                                                                                                 |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Development source | Browser applications register the official `@rsbuild/plugin-source-build`; it resolves `@orchbee/source`, compiles declared public source entries in the application graph, and lets the application's Rsbuild server own React and CSS hot replacement. |
| HMR process        | Package and application source edits share the native HMR graph; do not add package prebuilds, parallel library watchers, or a repository process supervisor.                                                                                            |
| Compiler plugins   | Register every plugin required by source-built packages. Source-building `@orchbee/ui`, for example, also requires its Tailwind CSS plugin because another workspace's Rslib configuration is not loaded.                                                |
| Production         | Register source building only for development. Production resolves `types`, `import`, `require`, and `default` and consumes Rslib output.                                                                                                                |
| Import visibility  | `@orchbee/source` changes how a declared public entry compiles, not which package paths an application may import.                                                                                                                                       |
| Root command       | Expose a public command such as `pnpm web` or `pnpm storybook`.                                                                                                                                                                                          |
| Application `dev`  | Keep it as the direct Rsbuild or Storybook command so the underlying tool owns CLI arguments and process lifecycle.                                                                                                                                      |

## Native application development

| Concern          | Contract                                                                                                                                                                              |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Toolchain        | Pin the exact Zig version in the owning build and CI setup; a different compiler version fails before producing artifacts.                                                            |
| Dependencies     | Declare native source dependencies by immutable URL and content hash in `build.zig.zon`; never require an untracked sibling checkout or machine-local path.                           |
| Workspace bridge | Use `package.json` scripts only to expose the native lifecycle to pnpm; do not add placeholder TypeScript source or `tsconfig.json`.                                                  |
| Formatting       | Run `zig fmt` through the application's `format` and `format:check` scripts.                                                                                                          |
| Testing          | Run native unit tests on the current platform and compile maintained operating-system targets through `test:cross`; cross-compilation proves compilation only, not platform behavior. |
| Output           | Keep `.zig-cache/` and `zig-out/` untracked. Release packaging consumes verified build output rather than committing it.                                                              |

A native application may resolve a content-addressed production dependency during its build. The
URL, digest, toolchain, and owning workspace must be explicit; mutable branches, unverified
archives, and implicit external checkouts remain forbidden.

## Dependency direction

- Applications consume packages only through declared exports.
- Packages and applications never import another application's source tree.
- Runtime workspaces never depend on development applications.
- Cross-workspace discovery of non-runtime artifacts requires an authoritative format and ownership contract; Storybook follows [`docs/frontend/storybook.md`](frontend/storybook.md).

## Product and development applications

A product application is part of an Orchbee user-facing runtime or distribution. A development application supports a repeatable repository workflow such as browsing, testing, or documenting UI code.

Development applications:

- set `private` to `true`;
- state their development-only role and product non-goals in `project.json`;
- participate in the local verification gate when their build validates tracked source;
- stay out of product packaging and release outputs;
- do not become a location for reusable runtime code.

Add a machine-readable product/development classification only when build, release, or policy automation needs it. Do not add metadata that merely repeats a workspace name and responsibility.

## Verification

`pnpm verify` builds and tests every application workspace that exposes the corresponding standard
scripts, type-checks applications with a TypeScript contract, and runs declared native formatting
and cross-compilation checks. Application build output is generated locally or in CI and remains
untracked.
