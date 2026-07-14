# Packages

## Scope and classification

- Put reusable code in `packages/*` when it has a workspace consumer or is intentionally developed as an independently consumable public library.
- Keep application-specific code under that application's `src` tree until a package boundary has a concrete consumer.
- Treat every runtime package under `packages/*` as a library package. Library packages use Rslib and expose built artifacts; consumers never compile another package's source.
- Set `"private": true` on workspace-only libraries. A package intended for npm publication omits `private` and must include a version, `"publishConfig": { "access": "public" }`, a package README, license metadata once the project license is selected, and a `files` allowlist.
- Record every package's responsibility and public boundary in [`docs/ARCHITECTURE.md`](ARCHITECTURE.md#current-modules).

Repository-only scripts and application entry points are not library packages and do not belong under `packages/*`.

## Naming

- Use the `@orchbee` npm scope and make the package name match its directory: `packages/<name>` is `@orchbee/<name>`.
- Use lowercase kebab-case nouns that name a stable domain or capability.
- Prefer the abstraction consumed by callers over an implementation technology, dependency, or current transport.
- Include a platform, vendor, or protocol in the name only when it defines a permanent public boundary.
- Do not create catch-all packages named `common`, `shared`, `utils`, `helpers`, or `misc`. Name the owned capability instead.
- Do not add `core` merely to make a name sound foundational. Use it only when sibling packages form an intentional family around that core.

```text
❌ packages/wasm-terminal  -> @orchbee/wasm-terminal
✅ packages/terminal       -> @orchbee/terminal
```

WASM and Ghostty implement the current terminal; callers depend on the terminal capability.

## Required package shape

Every library package contains these files:

| Path              | Responsibility                                      |
| ----------------- | --------------------------------------------------- |
| `package.json`    | Identity, scripts, dependencies, and public exports |
| `src/index.ts`    | Deliberately selected root public API               |
| `rslib.config.ts` | ESM, CJS, declarations, and runtime target          |
| `tsconfig.json`   | Package type-checking contract                      |
| `README.md`       | Consumer-facing installation and public API         |

Tests remain next to their source area according to [`docs/TESTING.md`](TESTING.md#source-and-test-paths). Generated build output belongs in `dist/` and is never committed.

Every library exposes these scripts:

| Script       | Command or responsibility       |
| ------------ | ------------------------------- |
| `build`      | `rslib build`                   |
| `dev`        | `rslib build --watch`           |
| `typecheck`  | `tsc --noEmit`                  |
| `test`       | Run the package test suite once |
| `test:watch` | Run package tests in watch mode |

## Public API and package exports

- Every package sets `"type": "module"`.
- Every package defines `package.json#exports`; files absent from `exports` are private even if they exist in the published tarball.
- Export the root as `"."`. Add each supported subpath explicitly, including assets. Do not use wildcard subpath exports.
- Put the `"types"` condition before `"import"` and `"require"`.
- Point `"import"` to ESM, `"require"` to CJS, and never point a public condition at `src/*`.
- Keep `main`, `module`, and `types` aligned with the root conditional export for consumers that do not fully support `exports`.
- Import another workspace package only through an allowed package name or exported subpath. Imports from another workspace's `src/*` or `dist/*` are forbidden.
- Document every public entry point and asset in the package README.

```json
{
  "name": "@orchbee/example",
  "type": "module",
  "main": "./dist/index.cjs",
  "module": "./dist/index.js",
  "types": "./dist/index.d.ts",
  "exports": {
    ".": {
      "types": "./dist/index.d.ts",
      "import": "./dist/index.js",
      "require": "./dist/index.cjs"
    },
    "./schema.json": "./schema.json"
  }
}
```

`src/index.ts` is an allowlist, not an automatic barrel:

```ts
// ❌ A new internal export silently becomes public.
export * from "./session-store.js";

// ✅ Values and types are reviewed independently.
export { SessionStore } from "./session-store.js";
export type { SessionSnapshot, SessionStoreOptions } from "./session-store.js";
```

Removing or changing an exported symbol, type, subpath, or asset is a breaking API change. Moving an internal file without changing `exports` is not.

## Library builds

- Build every library package with Rslib through `"build": "rslib build"`.
- Produce ESM (`dist/index.js`), CJS (`dist/index.cjs`), TypeScript declarations (`dist/index.d.ts`), and JavaScript source maps.
- Set the Rslib runtime target to the actual consumer environment: `web`, `node`, or a separately reviewed multi-target configuration.
- Use ES2022 syntax unless the repository runtime baseline changes.
- Generate declarations from exactly one Rslib output to avoid two formats writing competing declaration files.
- Keep Rslib's default externalization for `dependencies`, `optionalDependencies`, and `peerDependencies`. A deliberate bundled dependency belongs in `devDependencies` and must be explained in `rslib.config.ts` when the reason is not obvious.
- Keep published output unminified unless the package is a directly deployed browser artifact. Consumer applications own final minification and polyfills.
- Include only runtime artifacts, declarations, assets, and consumer documentation in `package.json#files`.

Use this baseline configuration and change it only for a documented package requirement:

```ts
import { defineConfig } from "@rslib/core";

export default defineConfig({
  lib: [
    {
      format: "esm",
      syntax: "es2022",
      dts: true,
      redirect: {
        dts: {
          extension: true,
        },
      },
    },
    { format: "cjs", syntax: "es2022" },
  ],
  output: {
    target: "node",
    sourceMap: true,
  },
});
```

When declarations contain relative imports, enable `redirect.dts.extension` on the declaration-producing output so extensionless TypeScript source emits Node-compatible declaration specifiers.

After changing a manifest or build configuration, run the package build and verify that both `import()` and `require()` resolve through `package.json#exports`.

## Dependency declarations

- Use `dependencies` for packages required whenever the library runs.
- Use `peerDependencies` when the consumer must provide one shared runtime instance or select a compatible framework version.
- Use `optionalDependencies` only when the library handles absence at runtime.
- Use `devDependencies` for build, test, type, and deliberately bundled implementation-only dependencies.
- Reference workspace libraries with `workspace:^` unless an exact lockstep dependency is required and documented.
- Declare every direct import in the importing package; do not rely on root or transitive dependencies.
- Put shared toolchain versions such as `@rslib/core`, TypeScript, and Vitest in the pnpm default catalog and reference them with `catalog:`.
- Keep production dependency ranges bounded to compatible versions and update the lockfile in the same change.
- Set `sideEffects: false` only after confirming the package has no import-time global registration, stylesheet import, polyfill, or other observable side effect. Use an explicit side-effect file list when only selected entry points have effects.

## Versioning

Orchbee packages follow Semantic Versioning with a Babel-style shared release line:

- All publishable `@orchbee/*` packages share one repository-wide major version. A package does not enter the next major independently.
- Publish only packages that changed. Unchanged packages keep their existing versions, so minor and patch versions do not have to match across the repository.
- After `1.0.0`, incompatible public API changes require the next coordinated major line, backward-compatible additions increment the affected package's minor version, and backward-compatible fixes increment its patch version.
- Before `1.0.0`, incompatible public API changes increment the affected package's minor version, backward-compatible features also increment minor, and fixes increment patch. Release notes must label breaking changes explicitly.
- A dependency-range change requires a release of the dependent package. Use at least a patch bump when its public API remains compatible; use the API-appropriate bump when dependency types or behavior leak through the public boundary.
- Deprecate public API before removing it when a practical migration path exists.
- Use `<version>-beta.<n>` for coordinated prereleases. Publish prereleases under a non-`latest` npm dist-tag; stable releases use `latest`.
- Release planning assigns each changed publishable package `major`, `minor`, `patch`, or `none`. The highest required bump wins for that package.

Renaming a published package creates a new package identity. Deprecate the old npm package with a migration message instead of publishing unrelated code under the old name.

## Verification and review

Before merging a package change:

1. Confirm the name and responsibility remain specific and non-overlapping.
2. Review `src/index.ts` and `package.json#exports` as public API changes.
3. Run `pnpm --filter <package-name> typecheck` and `pnpm --filter <package-name> test`.
4. Run `pnpm --filter <package-name> build` and smoke-test both ESM and CJS exports.
5. Run `pnpm lint`, `pnpm typecheck`, `pnpm test`, and `pnpm build` from the repository root.
6. Update the package README, architecture table, and affected engineering docs in the same change.

## External references

Verified on 2026-07-14:

- [Rslib output formats](https://rslib.rs/guide/basic/output-format)
- [Rslib configuration](https://rslib.rs/guide/basic/configure-rslib)
- [Rslib dependency externalization](https://rslib.rs/config/lib/auto-external)
- [Babel fixed-version release explanation](https://babeljs.io/docs/faq#why-isnt-a-certain-babel-x-package-updated)
- [Semantic Versioning 2.0.0](https://semver.org/)
