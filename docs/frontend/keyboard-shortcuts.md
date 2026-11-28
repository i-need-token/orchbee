# Frontend Keyboard Shortcuts

## Action ownership

A keyboard shortcut invokes a named semantic action under [`actions.md`](actions.md); it is not an
isolated keydown callback. The feature that owns the action also owns its default binding, binding
condition, tooltip presentation, shortcut tests, and removal lifecycle.

Choose the default mnemonic and its TUI counterpart together under the keyboard transfer rules in
[`INTERFACE-DESIGN.md`](../INTERFACE-DESIGN.md). Platform modifiers adapt the invocation mechanism;
the action identity, scope, and mnemonic remain recognizable across Clients.

Define bindings once in a responsibility-focused `*-shortcuts.ts` owner module. Components and lifecycle Hooks consume the descriptor or its matching functions instead of repeating key codes or modifier logic. Keep product bindings with their product owner until more than one workspace has a concrete need for the same shortcut engine.

## Binding contract

- Write the primary cross-platform modifier as `Mod` in engineering contracts. `Mod` maps to Meta on Apple platforms and Control elsewhere.
- Match letter shortcuts with `KeyboardEvent.code`, such as `KeyK`, so the physical binding remains stable across modifier-produced characters. Text-entry behavior that intentionally depends on the produced character uses `key` and documents that distinction.
- Match the complete modifier set. Reject unexpected Shift, Alt, Control, or Meta modifiers instead of accepting an ambiguous superset.
- Ignore composing and repeated keydown events unless the named action explicitly supports repetition.
- Call `preventDefault()` only after a currently available action matches. Do not call `stopPropagation()` unless a reviewed scope boundary requires it; a consumed sequence step is one, so neither step of a sequence reaches the focused widget.
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

## Sequences and widget keys

- A binding is one chord or a two-step sequence whose first step is exactly `Mod+K`. Sequences
  apply only while focus is in a scope that admits them; elsewhere, such as in a terminal, `Mod+K`
  keeps the meaning it has there.
- Where sequences apply, `Mod+K` is consumed whatever its second steps' availability and starts a
  waiting state. The next key other than a modifier is the second step: an available binding runs,
  and any other key is consumed and reported as naming no command. Waiting ends after five seconds,
  on window blur, or when a key arrives where no scope admits sequences. The status line announces
  waiting and a missed step in VS Code's wording.
- A tree or list widget may bind unmodified or Shift-only keys, which apply while focus is on the
  widget and not in a text field inside it. No binding takes Tab, and Escape only as a declared
  default that recording never captures.
- A region that owns keys, such as a side bar, an overlay, an editor, or a picker, keeps the
  modifier rule but may bind a function key alone or with Shift. Focus outside the region leaves
  the key to its native owner, such as a terminal's shell.
- A temporary surface that declares bindings, such as a picker, is the narrowest scope. Its keys win
  over every wider binding while focus is inside it, and every other key stays with the surface.
- A binding exists only on the platforms its owner gives a default. Where the platforms place a key
  differently, each placement is its own binding with its own platform defaults.
- Recording follows the row's scope: it accepts the keys that scope admits, and where sequences
  apply it records `Mod+K` and then a second step. Tab is never recorded; Escape or moving focus
  away cancels.
- Conflicts follow the resolution walk. The same keys in the same scope conflict and are refused. A
  narrower binding for a wider binding's keys is a shadow, which the narrower row notes. Keys no
  press could reach, such as a lone `Mod+K` where it starts a sequence, are refused.
- A code editor, or a tree or list widget, declares the keys it handles itself. While focus is
  inside it, and for a widget not in a text field inside it, the resolver yields those keys once the
  scope's own bindings have passed, and no wider scope takes them; `Mod+K` and a sequence's second
  step stay with the resolver.

## Display and localization

- Store one canonical binding and derive its display form. Do not maintain separate matching and tooltip strings.
- Display a sequence as its steps separated by a space, each in the platform chord format: `⌘K ⌘W` on Apple platforms and `Ctrl+K Ctrl+W` elsewhere.
- Display `Mod` as `⌘` on Apple platforms and `Ctrl` elsewhere. Use `Kbd` or `KbdGroup` for key presentation.
- Order Apple modifier glyphs as Control, Option, Shift, Command (`⌃⌥⇧⌘`), matching platform menu conventions.
- Localize the action name, not key codes, modifier glyphs, or platform names embedded in the chord formatter.
- A visible menu item, button tooltip, command result, and shortcut reference use the same action descriptor and platform formatter.
- A shortcut reference derives the action name, formatted chord, condition summary, and binding source from the registered descriptor rather than a separately maintained inventory.
- The control remains usable by pointer and assistive technology. A shortcut is an additional invocation path, never the only one.

## Native dialog affordances

- Unmodified Enter submission and Escape dismissal supplied by native form behavior and shared
  Dialog primitives are structural keyboard affordances, not application-wide shortcut bindings.
  Do not duplicate them in a product shortcut registry or attach parallel keydown listeners.
- Their visible `Kbd` hints follow [`dialogs-and-forms.md`](dialogs-and-forms.md#validation-and-actions).
  Render a hint from the current step's default editing context, hide it from the action's
  accessible name, and omit it whenever that mapping is conditional or ambiguous. Never intercept
  Enter to override native activation of an explicitly focused control.

## Lifecycle and verification

- Register browser listeners in a responsibility-focused lifecycle Hook or application composition owner and remove the exact listener during cleanup.
- Keep event normalization and descriptor matching pure so modifier, platform, composition, repeat, and non-match cases can be tested without React.
- A binding invokes the same Action and state transition as its visible control on Apple and non-Apple platforms, and leaves focus valid.
- A binding does not fire with unexpected modifiers, during composition, on a false context condition, or for a disabled Action; scope fallthrough, conflict precedence, and default prevention follow the declared rules.
- Document concrete product bindings in the owning package guide, not in this repository standard.
