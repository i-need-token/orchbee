# @orchbee/web

Browser composition entry for the Orchbee Workbench.

## Local development preview

`pnpm web` injects a stable set of prepared workspace-tab labels so the right-panel tab layout, selection, hover, constrained pointer reordering, closure, creation, pinning, grouping, group collapse, renaming, coloring, truncation, and responsive compression can be reviewed before an authoritative session source exists. The Web composition boundary owns the in-memory preview workspace state, keeps pinned tabs first and group members contiguous, atomically changes group membership when a tab crosses a group range, removes empty group metadata, creates localized untitled tab and group labels, and selects a neighboring visible tab after the active tab closes or its group collapses. `@orchbee/workbench` owns only the transient pointer-order projection and reports one final semantic move to the caller.

Production builds replace the development flag at compile time and pass an empty tab model. Remove the preview fixture when the browser application can prepare real tab data from its owning product capability.

The preview composition also applies completed tab-group moves atomically, keeping each group's members contiguous while its header is dragged in either expanded or collapsed state.
