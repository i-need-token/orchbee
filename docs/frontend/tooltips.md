# Frontend Tooltips

## Ownership and provider

Every runnable frontend installs one `TooltipProvider` from `@orchbee/ui` above its product interface. The shared provider owns hover timing and adjacent-tooltip behavior; feature components compose `Tooltip`, `TooltipTrigger`, and `TooltipContent` without installing another provider.

The provider's default open delay is `300ms`. Do not repeat that value at individual triggers. A different delay requires a documented interaction requirement and a focused test; visual preference alone is not an exception. Isolated Stories and tests install their own provider when they do not inherit an application decorator.

## Content contract

- Use a tooltip for a concise action label or clarification that supplements a control. Required instructions, state, validation, or error recovery remain visible without hover.
- Give every icon-only control an accessible name independently of its tooltip. Tooltip content does not replace `aria-label`, visible text, or another accessible naming relationship.
- Localize product-authored tooltip and accessibility text through `@orchbee/i18n`. Generic UI primitives receive consumer-provided text and never select product copy.
- When an action has a keyboard shortcut, show the localized action label followed by the platform-formatted shortcut using `Kbd` or `KbdGroup`. Do not place platform-specific glyphs inside translated messages.
- Keep tooltip text short and action-oriented. Do not put interactive controls, paragraphs, or product workflows inside a tooltip.
- Shared tooltip surfaces do not use a directional arrow. Placement and offset associate the surface with its trigger; consumers do not add an arrow locally.

## Interaction contract

- Pointer hover uses the shared `300ms` delay so incidental cursor movement does not create visual noise.
- Keyboard focus must reveal the same information and must not require pointer movement. Escape and focus departure close the tooltip according to the UI primitive's accessibility behavior.
- Touch users cannot depend on hover. A tooltip never carries information required to discover or complete an action.
- The trigger's hit target, disabled behavior, and focus indication remain owned by the underlying control. Do not add an invisible tooltip-only overlay that changes its interaction geometry.
- Place the tooltip where it does not cover the trigger or the immediate target of the action. Prefer the primitive's collision handling over viewport-specific coordinates.

## Verification

- Unit coverage fixes the `TooltipProvider` default at `300ms`.
- Representative browser interaction verification confirms that the tooltip remains closed before the delay and opens after it.
- Keyboard review reaches the trigger, exposes equivalent content, and closes it without trapping focus.
- Localization review covers every shipped locale and keeps shortcut glyph formatting outside message catalogs.
- Browser review checks placement, clipping, and contrast in the surfaces where the tooltip is consumed.
