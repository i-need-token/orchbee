# Frontend Visual Foundations

## Authority and scope

This document owns the selection rules for typography, text hierarchy, spacing, radius,
controls, icons, borders, and surfaces in Orchbee-authored product interfaces. Read it before
authoring or reviewing CSS in `apps/*` or `packages/*`. The registry and token-tier mechanics
remain authoritative in [`design-tokens.md`](design-tokens.md).

The compact desktop values below are the accepted light-theme baseline for the Workbench.
Theme-dependent values such as `#141414` are baseline mappings, not colors to repeat in component
CSS: product code consumes their `--ob-sys-*` roles and each theme supplies the concrete mapping.
Change a baseline only through the owning token, documentation, component, and Story in the same
reviewed change; do not introduce a local product override.

Tailwind replacement tokens and shadcn compatibility aliases do not expand the product
authoring vocabulary. New Orchbee-authored CSS uses only the semantic roles and approved
reference-token subsets defined here.

## Selection rules

- Choose typography and color by meaning, never by visual resemblance to another element.
- Use the smallest role that truthfully represents the content hierarchy; do not reduce a role
  merely to fit more content.
- Use one registered spacing or radius token. Raw values and calculations that create a new
  nominal value, such as an `11px` radius, are forbidden.
- Keep related items closer than unrelated items. An outer group gap must be at least as large as
  the gaps inside that group.
- Prefer `gap` for sibling relationships, container `padding` for insets, and `margin` only when
  neither owner can express the relationship.
- If no role fits, stop and propose a token change. Do not borrow a status color, compatibility
  alias, or unrelated size because its current value looks correct.

The style contract enforced by `pnpm lint:styles` rejects raw visual values and unregistered
custom properties. Semantic misuse still requires review against this document.

## Typography scale

Consume typography through the complete `--ob-sys-typography-*` shorthand. Do not combine a
font size from one role, line height from another, or a local font weight. HTML heading levels
express document structure; visual heading roles express presentation, so they do not need to
have matching suffixes.

| Variable                         | Baseline default | Use when                                                                 | Do not use when                                               |
| -------------------------------- | ---------------- | ------------------------------------------------------------------------ | ------------------------------------------------------------- |
| `--ob-sys-typography-body-sm`    | `400 13px/18px`  | Default dense desktop UI, navigation, setting rows, descriptions         | Long reading content needs a more relaxed line box            |
| `--ob-sys-typography-body-md`    | `400 14px/20px`  | Multi-line help, onboarding copy, and less dense product surfaces        | A compact toolbar or ordinary settings row                    |
| `--ob-sys-typography-body-lg`    | `400 16px/24px`  | Short introductory copy in an empty state or first-run experience        | Routine shell chrome or data-dense content                    |
| `--ob-sys-typography-label-sm`   | `400 12px/16px`  | Section labels and quiet category headings                               | Body descriptions or clickable control labels                 |
| `--ob-sys-typography-label-md`   | `500 13px/18px`  | Buttons, tabs, inputs, and compact controls needing explicit emphasis    | Page titles or explanatory paragraphs                         |
| `--ob-sys-typography-label-lg`   | `500 14px/20px`  | Prominent control labels and a primary panel toolbar                     | A substitute for a heading                                    |
| `--ob-sys-typography-heading-sm` | `500 16px/20px`  | Page title, settings title, or primary panel title                       | Every card or row title                                       |
| `--ob-sys-typography-heading-md` | `600 20px/28px`  | Major onboarding section or a page with stronger hierarchy               | Dense desktop settings or navigation                          |
| `--ob-sys-typography-heading-lg` | `600 24px/32px`  | Empty-state title or focused first-run step                              | Persistent workbench chrome                                   |
| `--ob-sys-typography-heading-xl` | `700 36px/40px`  | Rare hero or product-introduction statement                              | Ordinary application screens                                  |
| `--ob-sys-typography-code-sm`    | `400 12px/16px`  | Inline code, compact identifiers, and code-like metadata                 | Terminal cell rendering or normal prose                       |
| `--ob-sys-typography-code-md`    | `400 13px/18px`  | Code blocks, commands, logs, and terminal-adjacent read-only information | The interactive terminal canvas, which owns its font settings |

Apply these hierarchy rules:

- The shared UI stylesheet applies WebKit antialiasing and macOS Firefox grayscale smoothing at
  the page boundary. Components must not override font smoothing locally.
- Start with `body-sm` for compact desktop product UI.
- Keep a title and its supporting description at different roles or colors; do not make both
  larger.
- Use `label-*` for terse interface labels, not as a general method for making text bold.
- Use at most three visible type sizes in one persistent panel. Additional hierarchy should come
  from text color, spacing, or grouping.
- Do not shrink text to solve truncation. Wrap, truncate with an accessible full value, or change
  the layout.
- The terminal renderer's glyph size and line height are terminal preferences, not UI typography
  tokens.

## Text hierarchy

Text hierarchy describes information priority, not interaction state. Primary, secondary, and
tertiary text normally keep the same typography role within a row and differ by semantic color.
In the expressions below, `color @ N%` means
`color-mix(in srgb, color N%, transparent)`.

| Level         | Variable                          | Light baseline  | Use when                                                                  |
| ------------- | --------------------------------- | --------------- | ------------------------------------------------------------------------- |
| Heading       | `--ob-sys-color-text-heading`     | `#141414 @ 92%` | Page and panel headings                                                   |
| Primary       | `--ob-sys-color-text-primary`     | `#141414 @ 92%` | Task-critical content, row titles, values, user content, and control text |
| Secondary     | `--ob-sys-color-text-secondary`   | `#141414 @ 70%` | Descriptions and supporting copy directly attached to primary content     |
| Section label | `--ob-sys-color-text-label`       | `#141414 @ 70%` | Category and section labels                                               |
| Tertiary      | `--ob-sys-color-text-muted`       | `#141414 @ 48%` | Timestamps, low-priority metadata, hints, and de-emphasized context       |
| Placeholder   | `--ob-sys-color-text-placeholder` | `#141414 @ 32%` | Empty editable-control hints only                                         |
| Disabled      | `--ob-sys-color-text-disabled`    | Theme-mapped    | Unavailable content or controls                                           |
| Inverse       | `--ob-sys-color-text-inverse`     | Theme-mapped    | Text on an inverse semantic surface                                       |

Use primary text for the information a user must read to make the next decision. Use secondary
text when removing the copy would reduce understanding but not change the available action. Use
tertiary text when removing it would not block the task. Placeholder text is not a fourth
information level and must never carry persistent instructions or required data.

Do not set `opacity` on a text container to create hierarchy: it also fades icons, descendants,
focus indicators, and status colors. Apply the semantic color to the exact text or icon instead.
Status and link colors communicate meaning or interaction and must not replace a hierarchy level.

## Spacing scale

Only the spacing tokens below are approved for product-authored margin, padding, and layout gaps.
The same relationship uses the same token across components. Values above `32px` are page-level
tools and require visible whitespace to be an intentional part of the composition.

| Variable             | Value  | Use when                                                               |
| -------------------- | ------ | ---------------------------------------------------------------------- |
| `--ob-ref-space-0`   | `0`    | Joined edges, flush layout, or intentionally removing a gap            |
| `--ob-ref-space-0-5` | `2px`  | Hairline-adjacent optical correction or extremely tight icon detail    |
| `--ob-ref-space-1`   | `4px`  | Focus offset, tight icon detail, or compact vertical separation        |
| `--ob-ref-space-1-5` | `6px`  | Dense icon-label gap or inset where `8px` is visibly too loose         |
| `--ob-ref-space-2`   | `8px`  | Default icon-label gap, compact inset, or adjacent compact controls    |
| `--ob-ref-space-2-5` | `10px` | Compact row inset or intermediate separation in dense chrome           |
| `--ob-ref-space-3`   | `12px` | Compact stack gap, control group gap, or dense row horizontal inset    |
| `--ob-ref-space-4`   | `16px` | Default card/panel inset and standard gap between related blocks       |
| `--ob-ref-space-5`   | `20px` | Comfortable horizontal separation inside a row or toolbar              |
| `--ob-ref-space-6`   | `24px` | Panel/page gutter or separation between distinct groups in one section |
| `--ob-ref-space-8`   | `32px` | Separation between page sections                                       |
| `--ob-ref-space-10`  | `40px` | Page top inset or large break between unrelated regions                |
| `--ob-ref-space-12`  | `48px` | Centered composition or deliberately spacious page region              |
| `--ob-ref-space-16`  | `64px` | Empty-state breathing room or major first-run region                   |

Choose spacing by relationship:

- Within one atomic control, use `space-0-5` through `space-2`.
- Between elements of one component, start with `space-1-5`, `space-2`, or `space-3`.
- For a card or panel inset, start with `space-4`; use `space-6` only when the surface is not
  density-sensitive.
- Between related groups in one section, use `space-4` or `space-6`.
- Between independent sections, use `space-8` or more.
- A repeated list or stack uses `gap`; it does not give every child arbitrary bottom margin.
- Horizontal and vertical insets may differ when density requires it, but repeated instances of
  the same component must use the same pair.
- `calc()` may resolve structural geometry, such as subtracting a border from a fixed control,
  but must not manufacture a reusable visual value that is missing from this scale.

Spacing tokens do not double as arbitrary width or height tokens. Use control-size, icon-size,
container, or an owned `--ob-comp-*` geometry slot when the decision is size rather than space.

## Radius scale

New Orchbee-authored product CSS uses only this approved radius subset. Other registered radius
tokens exist for Tailwind or shadcn compatibility and are not additional design choices.

| Variable                | Value    | Use when                                                                  |
| ----------------------- | -------- | ------------------------------------------------------------------------- |
| `--ob-ref-radius-none`  | `0`      | Edge-to-edge panes, terminal surfaces, joined rows, and internal dividers |
| `--ob-ref-radius-sm`    | `4px`    | Tiny badges, compact tags, and subtle inline surfaces                     |
| `--ob-ref-radius-ui-sm` | `6px`    | Compact controls and small icon buttons                                   |
| `--ob-ref-radius-ui-md` | `8px`    | Standard inputs, buttons, and ordinary controls                           |
| `--ob-ref-radius-ui-lg` | `10px`   | Navigation selections, compact menus, and small floating surfaces         |
| `--ob-ref-radius-xl`    | `12px`   | Grouped settings cards and medium content cards                           |
| `--ob-ref-radius-2xl`   | `16px`   | Dialogs and large floating surfaces that need stronger separation         |
| `--ob-ref-radius-full`  | `9999px` | Avatars, status dots, circular controls, and intentional pills            |

Apply these radius rules:

- `11px`, `13px`, and every other unlisted radius are invalid even when they appear visually
  close to an approved value.
- Inputs and buttons use the radius owned by their UI component; consumers do not override it
  per screen.
- A grouped list rounds the outer container. Internal rows remain square so the group reads as
  one object.
- A nested visible surface uses the same radius as its parent or the next smaller approved radius;
  it never exceeds its parent radius.
- Use `radius-full` only when the geometry is actually circular or pill-shaped, not to avoid
  choosing a component radius.
- Do not add radius to an edge-to-edge terminal, sidebar, or main pane merely to make it look like
  a card.

## Control and icon sizes

Control and icon sizes are finite because inconsistent chrome makes the shell feel unstable.
Component consumers select a documented component `size` variant instead of assigning width or
height locally.

| Variable                   | Value  | Use when                                                                   |
| -------------------------- | ------ | -------------------------------------------------------------------------- |
| `--ob-ref-size-control-xs` | `24px` | Small button, inline action, compact icon button, or dense utility control |
| `--ob-ref-size-control-sm` | `28px` | Toolbar, sidebar navigation, compact select, or secondary desktop control  |
| `--ob-ref-size-control-md` | `32px` | Default button, input, select, and ordinary interactive control            |
| `--ob-ref-size-control-lg` | `36px` | Primary field, prominent action, or deliberately comfortable desktop input |
| `--ob-ref-size-icon-xs`    | `12px` | Status glyph, badge icon, compact metadata, or subordinate indicator       |
| `--ob-ref-size-icon-sm`    | `14px` | Default icon inside a compact control or sidebar item                      |
| `--ob-ref-size-icon-md`    | `16px` | Default navigation, field, toolbar, and button icon                        |
| `--ob-ref-size-icon-lg`    | `20px` | Prominent standalone action or empty-state support icon                    |

Use one icon size within a control. A dense `control-xs` or `control-sm` normally uses `icon-xs`
or `icon-sm`; a default `control-md` normally uses `icon-sm` or `icon-md`. An icon next to text
uses a `space-1-5` or `space-2` gap. Do not enlarge a decorative icon to create emphasis; promote
the action or text role instead. The visible control may be `24px` while its reusable component
provides a larger invisible pointer hit area; visual density and touch accessibility are separate
responsibilities.

## Borders and surfaces

Use `--ob-ref-border-width-default` (`1px`) for dividers and ordinary outlines. Use
`--ob-ref-border-width-strong` (`2px`) for focus geometry or a deliberately emphasized boundary.
Do not use a thicker product border without a component-specific requirement.

| Variable                                | Light baseline  | Use when                                              |
| --------------------------------------- | --------------- | ----------------------------------------------------- |
| `--ob-sys-color-border-subtle`          | `#141414 @ 5%`  | Divider inside one grouped object                     |
| `--ob-sys-color-border-default`         | `#141414 @ 12%` | Standard control outline or panel boundary            |
| `--ob-sys-color-border-strong`          | `#141414 @ 20%` | Boundary requiring more separation than default       |
| `--ob-sys-color-border-focus`           | `#2778c1`       | Keyboard-focus outline only                           |
| `--ob-sys-color-surface-canvas`         | `#fcfcfc`       | Application and page background                       |
| `--ob-sys-color-surface-raised`         | `#fcfcfc`       | Persistent panel above the canvas                     |
| `--ob-sys-color-surface-overlay`        | `#fcfcfc`       | Menu, popover, dialog, or temporary floating surface  |
| `--ob-sys-color-surface-subtle`         | `#141414 @ 4%`  | Quiet grouped card or inset region                    |
| `--ob-sys-color-surface-hovered`        | `#141414 @ 8%`  | Hover feedback for a neutral interactive region       |
| `--ob-sys-color-surface-selected`       | `#141414 @ 8%`  | Current navigation item or selected neutral region    |
| `--ob-sys-color-surface-selected-hover` | `#141414 @ 12%` | Hover feedback for an already selected neutral region |

Prefer a surface change or divider over a shadow in persistent shell chrome. Use
`--ob-sys-shadow-raised` for persistent raised content and `--ob-sys-shadow-overlay` for
temporary overlays only. A shadow does not replace the correct surface or border role.

## Actions and status

The light baseline uses `#2778c1` for primary action and focus, and `#007041` for success.
Primary action, selected navigation, and success are different meanings even when a reference
interface makes them visually similar.

| Variable                                | Light baseline  | Use when                                                      |
| --------------------------------------- | --------------- | ------------------------------------------------------------- |
| `--ob-sys-color-action-primary`         | `#2778c1`       | One primary action in the current context                     |
| `--ob-sys-color-action-primary-hover`   | `#2778c1 @ 90%` | Primary action hover state                                    |
| `--ob-sys-color-action-primary-pressed` | `#2778c1 @ 82%` | Primary action pressed state                                  |
| `--ob-sys-color-action-primary-text`    | `#fcfcfc`       | Text and icon on the primary-action surface                   |
| `--ob-sys-color-status-success`         | `#007041`       | Successful result or a state whose meaning is explicitly good |

A generic binary switch does not use success merely because it is on. It uses the selected or
component action role unless its enabled state explicitly communicates success. Warning, danger,
and info roles similarly communicate status and never serve as arbitrary accents.

## Composition measurements

The current settings composition evaluates a `192px` navigation column, a `600px` maximum
content column, `52px` minimum rows, `40px` page-top inset, and `12px` grouped-card radius. Width
and row-height values are owned by that composition and are not additions to the global spacing
scale.

Use `min-height` for settings rows so localization, zoom, and wrapped descriptions may increase
the height. Use intrinsic grid or flex sizing below the maximum content width. When a reusable
settings shell or row becomes part of `@orchbee/workbench`, expose only the geometry that must be
customizable through registered `--ob-comp-*` tokens; do not reuse a spacing or container token
under an unrelated meaning.

## Review checklist

Before accepting an authored interface change, verify all of the following:

1. Every text element has one semantic typography role and one semantic text/status role.
2. Primary, secondary, and tertiary text reflect information priority rather than arbitrary
   contrast preference.
3. Every gap, margin, and padding value belongs to the spacing scale and represents the stated
   relationship.
4. Every radius belongs to the approved product subset and matches the component category.
5. Controls and icons use component size variants rather than local dimensions.
6. Borders, actions, statuses, and surfaces use semantic variables instead of reference colors.
7. Repeated components use identical values, including across empty, hover, selected, focus, and
   disabled states.
8. `pnpm lint:styles` and the repository `pnpm verify` gate pass.
