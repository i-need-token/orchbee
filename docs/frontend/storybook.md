# Storybook

## Repository model

| Location                               | Ownership                                                                                                  |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `apps/storybook/.storybook/`           | The single repository Storybook host: framework, Rsbuild, addons, preview, static build, and browser entry |
| `packages/<frontend-package>/stories/` | Stories owned by a shared frontend package                                                                 |
| `apps/<visual-application>/stories/`   | Application-only visual behavior that cannot belong to a reusable package                                  |

`apps/storybook` owns no component or product behavior. Do not move its configuration into `packages/ui` or create one Storybook per workspace by default.

## Story ownership

- The workspace that owns rendered behavior owns its `<component>.stories.tsx` file beside `src/` in top-level `stories/`.
- Every public visual component has a default Story; add applicable disabled, loading, empty, error, open, selected, variant, and responsive states.
- Pure types, nonvisual Hooks, and internal provider plumbing need no standalone Story.
- Update owned Stories with every public visual behavior change.
- A canonical experience-component Manifest references an owned deterministic Story and the states
  to inspect; update its Story ID and Manifest atomically under
  [`experience-components.md`](experience-components.md).
- Import other workspaces only through package exports, never `src/*` or `dist/*`.
- Keep fixtures deterministic and independent of live networks, current time, randomness, credentials, and machine state.

## Navigation contract

| Title prefix          | Owner                                |
| --------------------- | ------------------------------------ |
| `UI`                  | `@orchbee/ui` reusable primitives    |
| `Packages/<name>`     | Another shared frontend package      |
| `Applications/<name>` | Application-specific visual behavior |

Story titles, export names, and generated IDs are browser-automation interfaces; update every link and browser test when renaming them.

- Use tags for cross-cutting status or ownership filters.
- Keep global decorators generic; owner-specific providers, mocks, and layouts stay with the owning Stories.

## Hosting decision

Keep one physical Storybook while Stories share a renderer, browser environment, dependency set, and preview contract. Story count alone never justifies a split.

Split only for measured unacceptable build or test cost, incompatible renderers or runtimes, dependencies that cannot coexist, or independent ownership and deployment. Preserve one discovery entry through Storybook Composition and document measurements before setting a numerical threshold.

## Authoring and verification

- Keep one primary component module per Component Story Format file.
- Prefer explicit args and fixtures; use `play` for required interaction.
- Use Storybook layout and viewport parameters before Story-only CSS.
- Story-only layout must not restyle the subject.
- Import an owning package stylesheet through its public asset export.

Run from the repository root:

```sh
pnpm storybook
pnpm storybook:build
```

`pnpm storybook` starts one process; Rsbuild includes declared public source entries in native HMR without parallel package watchers. The static build runs through the Storybook workspace `build` task in `pnpm verify`, and its workspace test requires a Story for every public `@orchbee/ui` component module.

`pnpm storybook:build` and the root build finish shared package builds before starting the static host. Discovered application Stories can import packages absent from the host's direct dependency list; a clean static build needs those package exports before Storybook starts.

Browser agents discover a Story in the sidebar, then use `iframe.html?id=<story-id>&viewMode=story` for DOM, interaction, accessibility, computed-style, and screenshot checks.

Primary references: [Story loading](https://storybook.js.org/docs/configure/index), [tags](https://storybook.js.org/docs/writing-stories/tags), [Composition](https://storybook.js.org/docs/sharing/storybook-composition), and [Rsbuild integration](https://storybook.rsbuild.dev/guide/).
