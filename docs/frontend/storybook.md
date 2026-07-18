# Storybook

## Repository model

`apps/storybook` is Orchbee's single repository-wide Storybook host. It owns the Storybook framework version, Rsbuild integration, addons, global preview configuration, static build, and browser entry point. It does not own components or product interface behavior.

Frontend workspaces own their Story files in a top-level `stories/` directory beside `src/`:

```text
apps/storybook/.storybook/
packages/ui/stories/
packages/<frontend-package>/stories/
apps/<visual-application>/stories/
```

Do not put the repository Storybook configuration in `packages/ui`, and do not create one Storybook installation per workspace by default. Applications contribute Stories only for visual behavior that cannot live in a reusable frontend package.

## Story ownership

Each Story is maintained by the workspace that owns the rendered behavior. A change to a public visual component updates its owned Stories in the same change.

Every public visual component module has at least one Story covering its default rendered state. Add the applicable variants, disabled, loading, empty, error, open, selected, and responsive states. Pure types, non-visual hooks, and internal provider plumbing do not require standalone Stories.

Stories import other workspaces only through declared package exports. They never import another workspace's `src/*` or `dist/*` paths. Fixtures and mocks remain deterministic and owned by the Story's workspace; do not depend on live networks, the current time, random data, credentials, or machine-specific state.

## Navigation contract

Top-level Story titles identify the owning surface:

| Prefix                | Owner                                |
| --------------------- | ------------------------------------ |
| `UI`                  | `@orchbee/ui` reusable primitives    |
| `Packages/<name>`     | Another shared frontend package      |
| `Applications/<name>` | Application-specific visual behavior |

Treat Story titles, export names, and generated IDs as browser automation interfaces. Rename them deliberately and update every documented link or browser test in the same change.

Use Storybook tags for cross-cutting status or ownership filters. Keep global preview decorators generic. A provider, mock, or layout required by only one workspace belongs with that workspace's Stories rather than behind owner-specific branches in the global preview.

## Single-instance default

Story count alone is not a reason to split Storybook. Keep one physical instance while Stories share a compatible renderer, browser environment, dependency set, and global preview contract.

Split an independently built Storybook only when measured build or test cost is unacceptable, renderers or required runtimes are incompatible, dependencies cannot coexist, or ownership and deployment genuinely require isolation. Preserve one discovery entry by composing split Storybooks through Storybook Composition.

Do not set numerical split thresholds without repository measurements that demonstrate the threshold predicts a real development or CI problem.

## Authoring constraints

- Use Component Story Format files named `<component>.stories.tsx`.
- Keep one primary component module per Story file so coverage is mechanically discoverable.
- Prefer explicit, readable fixture values and args over generated data.
- Use `play` functions for behavior that requires interaction.
- Use Storybook layout and viewport parameters before adding Story-only CSS.
- Story-only layout must not restyle the component under inspection.
- Import the owning package stylesheet through its public asset export.

## Commands and verification

Run the repository Storybook from the repository root:

```sh
pnpm storybook
pnpm storybook:build
```

The static Storybook build is part of the application workspace `build` task and therefore part of `pnpm verify`. The Storybook workspace test checks that every public `@orchbee/ui` component module has a matching Story file.

Browser agents open the development server, discover a Story through the sidebar, and use the isolated `iframe.html?id=<story-id>&viewMode=story` route for DOM, interaction, accessibility, computed-style, and screenshot checks.

## External references

Verified on 2026-07-17:

- [Story loading configuration](https://storybook.js.org/docs/configure/index) defines multi-directory Story discovery and title prefixes.
- [Story tags](https://storybook.js.org/docs/writing-stories/tags) defines sidebar and test filtering.
- [Storybook Composition](https://storybook.js.org/docs/sharing/storybook-composition) defines aggregation across independently built Storybooks and renderers.
- [Storybook Rsbuild](https://storybook.rsbuild.dev/guide/) defines the Storybook 10 and Rsbuild integration used by the repository host.
