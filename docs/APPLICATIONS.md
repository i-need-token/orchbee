# Applications

## Placement

Put an independently runnable or deployable entry point in `apps/*`. Application workspaces include both end-user products and development applications; audience does not change placement.

Put reusable capabilities consumed through imports in `packages/*`. Put repository automation that has no independently running application lifecycle in `scripts/*`.

```text
apps/
  <product-surface>/  # End-user product application
  <development-tool>/ # Repository development application
```

Do not create a new top-level directory to distinguish an internal application from a product application. Express that distinction through the workspace name, `private` manifest field, `project.json` responsibility and non-goals, and release configuration when a release pipeline exists.

Name a product application for the user-facing surface or runtime role it owns, not an internal
process such as `main` or `renderer`. Create a headless long-running application only when that
entry point has a concrete lifecycle and consumer; an architecture diagram is not an admission
reason.

## Required application shape

Every direct application workspace contains:

| Path            | Responsibility                                     |
| --------------- | -------------------------------------------------- |
| `package.json`  | Identity, commands, and dependencies               |
| `project.json`  | Runtime, stability, responsibility, and non-goals  |
| `tsconfig.json` | Type-checking contract for TypeScript applications |

An application exposes the scripts required by its lifecycle. Browser applications normally expose `dev`, `build`, `typecheck`, `test`, and `test:watch`. A specialized development application may add explicit workflow commands, but must keep `build` non-interactive so `pnpm verify` can run it.

## Workspace development

Browser applications register the official `@rsbuild/plugin-source-build` plugin for their
development command. The plugin resolves the `@orchbee/source` condition declared by workspace
packages, compiles those public source entry points in the application graph, and lets the
application's Rsbuild server own React and CSS hot replacement. A package source edit therefore
flows through the same native HMR graph as an application source edit; no package prebuild,
parallel library watcher, or repository process supervisor is required.

The application must register every compiler plugin required by source-built packages. For
example, applications that source-build `@orchbee/ui` also register the Tailwind CSS plugin used
by that package; source building does not load another workspace's Rslib configuration.

Register source building only for development. Production application builds resolve the normal
`types`, `import`, `require`, and `default` package conditions and consume Rslib output. The
source condition changes how a declared public entry point is compiled, not which package paths
an application may import.

Run a public root command such as `pnpm web` or `pnpm storybook`. Keep the
application's `dev` script as the direct Rsbuild or Storybook command so CLI arguments and process
lifecycle remain owned by the underlying tool.

## Dependency direction

Applications may consume packages only through declared package exports. Packages and applications never import from another application's source tree. No runtime workspace depends on a development application.

An application may discover non-runtime artifacts such as Story files from another workspace only when an authoritative repository contract defines the artifact format and ownership. Storybook follows the dedicated contract in [`docs/frontend/storybook.md`](frontend/storybook.md).

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

`pnpm verify` builds, type-checks, and tests every application workspace that exposes the corresponding standard scripts. Application build output is generated locally or in CI and remains untracked.
