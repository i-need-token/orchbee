# Frontend Keyboard Shortcuts

## Action ownership

A keyboard shortcut invokes a named semantic action; it is not an isolated keydown callback. The feature that owns the action owns its stable identifier, default binding, availability rules, invocation, tooltip presentation, tests, and removal lifecycle.

Define bindings once in a responsibility-focused `*-shortcuts.ts` owner module. Components and lifecycle Hooks consume the descriptor or its matching functions instead of repeating key codes or modifier logic. Keep product bindings with their product owner until more than one workspace has a concrete need for the same shortcut engine.

## Binding contract

- Write the primary cross-platform modifier as `Mod` in engineering contracts. `Mod` maps to Meta on Apple platforms and Control elsewhere.
- Match letter shortcuts with `KeyboardEvent.code`, such as `KeyK`, so the physical binding remains stable across modifier-produced characters. Text-entry behavior that intentionally depends on the produced character uses `key` and documents that distinction.
- Match the complete modifier set. Reject unexpected Shift, Alt, Control, or Meta modifiers instead of accepting an ambiguous superset.
- Ignore composing and repeated keydown events unless the named action explicitly supports repetition.
- Call `preventDefault()` only after a currently available action matches. Do not call `stopPropagation()` unless a reviewed scope boundary requires it.
- Platform detection belongs at the application composition boundary. Product packages receive a validated platform category and do not read `navigator` or operating-system globals.

## Scope and conflicts

Resolve a key event from the narrowest active scope outward:

1. An open modal, menu, command surface, or other temporary focus scope.
2. The focused editor, terminal, or feature surface.
3. The mounted product shell.
4. The application-wide scope.
5. Browser and operating-system behavior that the application has not explicitly reserved.

Global shortcuts must use a primary modifier and represent actions that remain valid while focus is inside ordinary product content. Do not override unmodified typing, text editing, input-method composition, or terminal input for a convenience action. When a global binding deliberately overrides a familiar browser or editing command, document the product reason and expose the same action through visible UI.

Before adding a binding, search all scopes for the same canonical chord and resolve the conflict in the owning design. First-match listener order is not a conflict policy.

## Display and localization

- Store one canonical binding and derive its display form. Do not maintain separate matching and tooltip strings.
- Display `Mod` as `⌘` on Apple platforms and `Ctrl` elsewhere. Use `Kbd` or `KbdGroup` for key presentation.
- Localize the action name, not key codes, modifier glyphs, or platform names embedded in the chord formatter.
- A visible menu item, button tooltip, command result, and shortcut reference use the same action descriptor and platform formatter.
- The control remains usable by pointer and assistive technology. A shortcut is an additional invocation path, never the only one.

## Lifecycle and verification

- Register browser listeners in a responsibility-focused lifecycle Hook or controller and remove the exact listener during cleanup.
- Keep event normalization and descriptor matching pure so modifier, platform, composition, repeat, and non-match cases can be tested without React.
- Test every default binding on Apple and non-Apple platform categories, including unexpected modifiers, composition, repeat, prevention behavior, and disabled action state.
- Browser verification invokes the visible control and the keyboard chord, confirms the same state transition, and checks that focus remains valid.
- Document concrete product bindings in the owning package guide, not in this repository standard.
