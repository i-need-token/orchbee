import type { ActionScope } from '@orchbee/actions';
import type { KeyboardShortcutChord } from '@orchbee/keyboard-shortcuts';
import * as React from 'react';

import { WorkbenchKeyboardScopeContext } from './workbench-keyboard-scope-context';
import type { WorkbenchKeyboardScope } from './WorkbenchKeyboardScopeRegistry';

/**
 * Makes the element `reference` holds own `scope`'s keys while the calling component is mounted, at
 * every key press the element is rendered for; null registers nothing, for a component that owns
 * keys only where its owner asks. Declare `scope` once, as a module constant, or keep its `yields`
 * the same array across renders.
 *
 * ```tsx
 * const EXPLORER_KEYBOARD_SCOPE: WorkbenchKeyboardScope = { kind: 'widget', widget: 'explorer' };
 *
 * const treeReference = React.useRef<HTMLDivElement>(null);
 * useWorkbenchKeyboardScope(treeReference, EXPLORER_KEYBOARD_SCOPE);
 * return <div ref={treeReference} role='tree'>…</div>;
 * ```
 */
export function useWorkbenchKeyboardScope(
	reference: React.RefObject<Element | null>,
	scope: WorkbenchKeyboardScope | null,
): void {
	const registry = React.useContext(WorkbenchKeyboardScopeContext);
	const kind = scope?.kind;
	const actionScope = scope?.actionScope;
	const widget = scope?.kind === 'widget' ? scope.widget : undefined;
	const yields =
		scope?.kind === 'code-editor' || scope?.kind === 'widget' ? scope.yields : undefined;
	React.useEffect(() => {
		if (registry === null || kind === undefined) return undefined;
		return registry.register(reference, registeredScope(kind, actionScope, widget, yields));
	}, [actionScope, kind, reference, registry, widget, yields]);
}

/** Rebuilds the scope from the values the effect depends on, so a new object each render is harmless. */
function registeredScope(
	kind: WorkbenchKeyboardScope['kind'],
	actionScope: ActionScope | undefined,
	widget: string | undefined,
	yields: readonly KeyboardShortcutChord[] | undefined,
): WorkbenchKeyboardScope {
	switch (kind) {
		case 'code-editor':
			return { actionScope, kind, yields: yields ?? [] };
		case 'widget':
			return { actionScope, kind, widget: widget ?? '', yields };
		case 'file-bar':
		case 'file-overlay':
		case 'quick-input':
			return { actionScope, kind };
	}
}
