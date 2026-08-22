# Frontend Keyboard Shortcuts

## Action ownership

A keyboard shortcut invokes a named semantic action under [`actions.md`](actions.md); it is not an
isolated keydown callback. The feature that owns the action also owns its default binding, binding
condition, tooltip presentation, shortcut tests, and removal lifecycle.

Define bindings once in a responsibility-focused `*-shortcuts.ts` owner module. Components and lifecycle Hooks consume the descriptor or its matching functions instead of repeating key codes or modifier logic. Keep product bindings with their product owner until more than one workspace has a concrete need for the same shortcut engine.

## Binding contract

- Write the primary cross-platform modifier as `Mod` in engineering contracts. `Mod` maps to Meta on Apple platforms and Control elsewhere.
- Match letter shortcuts with `KeyboardEvent.code`, such as `KeyK`, so the physical binding remains stable across modifier-produced characters. Text-entry behavior that intentionally depends on the produced character uses `key` and documents that distinction.
- Match the complete modifier set. Reject unexpected Shift, Alt, Control, or Meta modifiers instead of accepting an ambiguous superset.
- Ignore composing and repeated keydown events unless the named action explicitly supports repetition.
- Call `preventDefault()` only after a currently available action matches. Do not call `stopPropagation()` unless a reviewed scope boundary requires it.
- Platform detection belongs at the application composition boundary. Product packages receive a validated platform category and do not read `navigator` or operating-system globals.

## Context and availability

A binding may declare an optional context condition, conventionally called `when`, that determines whether its chord routes to its action. Evaluate the condition as pure logic against one read-only context snapshot for the key event. A false condition skips that binding so another binding, an outer scope, or native behavior can handle the event.

Context values describe semantic product state such as the active focus scope, open interaction surface, selection state, editing mode, or available capability. The owner of that state publishes and clears the value with its lifecycle. Binding resolution must not infer context by querying incidental DOM structure, CSS selectors, or localized text.

Action availability is separate from binding applicability:

- `when` determines whether one binding participates in routing and conflict resolution.
- The action availability contract in [`actions.md`](actions.md#availability-and-confirmation)
  determines whether the action may run from any invocation path, including menus, commands,
  pointer controls, automation, and shortcuts.
- A resolver invokes and consumes the key event only when both the binding condition and action availability pass. Otherwise it continues resolution without calling `preventDefault()`.

Do not introduce a shared string expression language, global context-key inventory, or user-editable condition syntax before concrete product consumers require them. Product-local conditions must still be pure, deterministic, testable, and represented by inspectable metadata when a shortcut reference displays them; UI must not derive a condition label from function source.

## Scope and conflicts

Resolve a key event from the narrowest active scope outward:

1. An open modal, menu, command surface, or other temporary focus scope.
2. The focused editor, terminal, or feature surface.
3. The mounted product shell.
4. The application-wide scope.
5. Browser and operating-system behavior that the application has not explicitly reserved.

Global shortcuts must use a primary modifier and represent actions that remain valid while focus is inside ordinary product content. Do not override unmodified typing, text editing, input-method composition, or terminal input for a convenience action. When a global binding deliberately overrides a familiar browser or editing command, document the product reason and expose the same action through visible UI.

Binding resolution operates over an enumerable binding set, not the incidental order of independent browser listeners. For one canonical chord, evaluate active scopes in the order above and skip candidates whose context or action availability does not pass. Stop after the first valid candidate consumes the event.

Before adding a binding, search all scopes for the same canonical chord. Conditions for bindings in the same scope must be mutually exclusive; otherwise the owner must declare and test a stable precedence. Registration order and first-match listener order are not conflict policies.

## Display and localization

- Store one canonical binding and derive its display form. Do not maintain separate matching and tooltip strings.
- Display `Mod` as `⌘` on Apple platforms and `Ctrl` elsewhere. Use `Kbd` or `KbdGroup` for key presentation.
- Order Apple modifier glyphs as Control, Option, Shift, Command (`⌃⌥⇧⌘`), matching platform menu conventions.
- Localize the action name, not key codes, modifier glyphs, or platform names embedded in the chord formatter.
- A visible menu item, button tooltip, command result, and shortcut reference use the same action descriptor and platform formatter.
- A shortcut reference derives the action name, formatted chord, condition summary, and binding source from the registered descriptor rather than a separately maintained inventory.
- The control remains usable by pointer and assistive technology. A shortcut is an additional invocation path, never the only one.

## Lifecycle and verification

- Register browser listeners in a responsibility-focused lifecycle Hook or application composition owner and remove the exact listener during cleanup.
- Keep event normalization and descriptor matching pure so modifier, platform, composition, repeat, and non-match cases can be tested without React.
- Test every default binding on Apple and non-Apple platform categories, including unexpected modifiers, composition, repeat, true and false context conditions, disabled action state, scope fallthrough, conflict precedence, and prevention behavior.
- Browser verification invokes the visible control and the keyboard chord, confirms the same state transition, and checks that focus remains valid.
- Document concrete product bindings in the owning package guide, not in this repository standard.
