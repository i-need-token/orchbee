# @orchbee/web

Browser composition entry for the Orchbee Workbench.

## Local development preview

`pnpm web` renders the Workbench shell with its localized sidebar, host-owned terminal tabs, and
per-tab panel workspace. The browser composition owns locale and shortcut-platform setup; PTY
rendering, terminal search, and other host capabilities will be composed into the shell when their
ownership contracts are defined.
