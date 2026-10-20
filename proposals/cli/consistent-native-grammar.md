# Consistent native CLI grammar

## Purpose and status

Delivery decisions for ORC-136 in the authorized Todo batch. This record explains the choices
and coordinated rollout; shipped behavior belongs to the [command-surface standard](../../docs/cli/command-surface.md)
and [native CLI reference](../../apps/orchbee/docs/native-cli-query.md). It is retained as rationale
for future grammar changes, not as a second runtime authority.

## Decisions

| Area               | One public spelling and meaning                                                                                                                                                                                                                                                        |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Cardinal direction | `--direction left                                                                                                                                                                                                                                                                      | right      | up               | down` for Panel geometry and placement; map to existing wire directions at the boundary.                                     |
| Ordered placement  | `tab move --order previous                                                                                                                                                                                                                                                             | next       | first            | last`, exclusive with `--insert-index`.                                                                                      |
| Search direction   | `terminal copy-search --search-direction forward                                                                                                                                                                                                                                       | backward`. |
| Deletion verbs     | `close` ends live workspace/tab/panel lifecycle; `delete` destroys a stored record; `remove` detaches managed filesystem/machine membership; `uninstall` removes an installed plugin; `dissolve` removes grouping while preserving children. Add these effect classes to the standard. |
| Panel target       | Positional `[panel]` in the Panel family, optionally `--current` for contextual selection. Remove synonymous `--panel` there. Other families retain typed `--panel` selectors.                                                                                                         |
| Agent target       | Preserve exact Session/name selection and add explicit `--terminal` / `@terminal:` targeting alongside `--panel` / `@panel:`. A terminal target can identify a child; a Panel selects its primary. Never fall back to another member.                                                  |
| Names              | Keep `--label` for creation/rename and `--name` for managed Agent identity. A new label is not an additional target operand.                                                                                                                                                           |
| Provider           | Use `--provider` throughout, canonical interactive provider names including `antigravity`. Integration capability is a supported subset; private adapter ID `antigravity-cli` remains at the adapter boundary.                                                                         |
| Event names        | CLI event input/output uses `panel.*`; map existing private `pane.*` wire events at the CLI boundary. Version the public event envelopes and preserve old schema versions.                                                                                                             |
| Output             | `--source visible                                                                                                                                                                                                                                                                      | recent     | recent-unwrapped | detection`plus`--lines`; remove read/wait `--history`, `--rows`, `--unwrap`. Geometry `--rows` remains a terminal dimension. |
| Wait budget        | `terminal wait-output` defaults to 30000 ms, accepts bounded `--timeout`, and uses `--forever` only for explicit unbounded waiting.                                                                                                                                                    |
| Keys               | Modifier spelling `ctrl+`, `alt+`, `shift+`, `super+`; named keys use their canonical lowercase names. Remove hyphen/control/OS-name aliases while preserving printable scalar input.                                                                                                  |

## Rollout and invariants

This experimental command surface changes directly in one coordinated build: obsolete spellings
fail before runtime access. No silent aliases, automatic retry or changed destination inference is
introduced. Stored runtime identities, Server mutation ordering, and private adapter/wire DTOs
keep their existing contracts. Pure boundary translations carry public naming differences.

The grammar inventory, completion, localized help, Agent guide, public schemas and paired package
references change together. Parser and completion cases cover accepted and rejected spellings;
projection cases cover exact Panel/Terminal routing and event-name translation. All implementation
in the batch is committed issue by issue, then checked together at the user's requested final gate.
