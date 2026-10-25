# Terminal correctness evidence

Client-owned async workflows use controlled implementation tests. The shared cross-process attachment protocol is checked separately by the Desktop `projected-terminal-ipc-reliability` model; no local UI workflow needs its own TLA model merely because it awaits work.

## Claims and implementation evidence

Paths are relative to the package root. These are maintained test obligations, not a claim of exhaustive exploration.

| Claim                                                                   | Executable evidence                                                                                                    |
| ----------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Renderer retirement, device loss and visibility fence every completion  | `src/rendering/__tests__/WebGPURenderer.unit.test.ts`                                                                  |
| Only the newest live presentation configuration publishes all fields    | `src/projected/__tests__/projected-terminal.unit.test.ts`                                                              |
| Late image reads cannot revive a stale preview                          | `src/projected/__tests__/TerminalImagePreviewController.unit.test.ts`                                                  |
| Input release and repair preserve ordering and ownership                | `src/input/__tests__/terminal-input-state.unit.test.ts`, `src/input/__tests__/OrderedInputDispatcher.unit.test.ts`     |
| Row reads, acknowledgement and request retirement preserve current view | `src/projected/__tests__/projected-terminal.unit.test.ts`, `src/projected/__tests__/projected-scrollback.unit.test.ts` |
| Search revisions, watchdogs and navigation reject stale completion      | `src/search/__tests__/projected-search-controller.unit.test.ts`                                                        |
| Prompt navigation commits only the current request                      | `src/projected/__tests__/projected-prompt-navigation.unit.test.ts`                                                     |
| Selection gestures and native input settle without replay               | `src/projected/__tests__/projected-terminal.unit.test.ts`, `src/input/__tests__/surface-mouse.unit.test.ts`            |

Tests control completion order, supersession, failure and disposal at the production boundary. Keep concrete payload, geometry, clock and rendering assertions here. They cover named and generated traces; they do not establish all possible external schedules or browser/GPU behavior.

## Verification

Run `pnpm --filter @orchbee/terminal test`. Changes to the shared attachment protocol also require its registered Desktop model and adapter tests. The repository root gate remains required; this package has no `test:tla` command.
