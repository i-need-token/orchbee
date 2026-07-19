# @orchbee/web

Browser composition entry for the Orchbee Workbench.

## Local development preview

`pnpm web` injects a stable set of prepared workspace-tab labels so the right-panel tab layout, selection, hover, drag reordering, closure, creation, truncation, and responsive compression can be reviewed before an authoritative session source exists. The Web composition boundary owns the in-memory preview state, applies completed tab moves, creates localized untitled labels, and selects a neighboring tab after the active tab closes; `@orchbee/workbench` only reports semantic actions.

Production builds replace the development flag at compile time and pass an empty tab model. Remove the preview fixture when the browser application can prepare real tab data from its owning product capability.
