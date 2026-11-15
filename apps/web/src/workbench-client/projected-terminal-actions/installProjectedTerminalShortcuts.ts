import type { ActionScope } from '@orchbee/actions';

import {
	defaultProjectedTerminalShortcutBindings,
	matchesProjectedTerminalSelectionCopyKey,
	PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS,
	resolveProjectedTerminalShortcut,
	type ProjectedTerminalShortcutBinding,
	type ProjectedTerminalShortcutBindingSource,
	type ProjectedTerminalShortcutFocus,
	type ProjectedTerminalShortcutPlatform,
} from '../../projected-terminal-keyboard-shortcuts';
import type { ProjectedTerminalShortcutInvokeOptions } from './createProjectedTerminalActionComposition';

export interface InstallProjectedTerminalShortcutsOptions {
	/** Windows: Ctrl+C copies and clears the focused terminal's selection before it can interrupt. */
	readonly ctrlCCopiesSelection?: boolean | undefined;
	readonly document: Document;
	readonly findRoot: HTMLElement;
	readonly invoke: (
		actionId: ProjectedTerminalShortcutBinding['actionId'],
		input: undefined,
		scope: ActionScope,
		options?: ProjectedTerminalShortcutInvokeOptions,
	) => boolean;
	readonly findScope: ActionScope;
	readonly platform: ProjectedTerminalShortcutPlatform;
	readonly shortcutBindingSource?: ProjectedTerminalShortcutBindingSource | undefined;
	readonly terminalRoot: HTMLElement;
	readonly terminalScope: ActionScope;
}

export function installProjectedTerminalShortcuts({
	ctrlCCopiesSelection = false,
	document,
	findRoot,
	findScope,
	invoke,
	platform,
	shortcutBindingSource,
	terminalRoot,
	terminalScope,
}: InstallProjectedTerminalShortcutsOptions): () => void {
	const focusFor = (target: EventTarget | null): ProjectedTerminalShortcutFocus => {
		const NodeConstructor = findRoot.ownerDocument.defaultView?.Node;
		if (NodeConstructor === undefined || !(target instanceof NodeConstructor)) return 'outside';
		if (findRoot.contains(target)) return 'find';
		return terminalRoot.contains(target) ? 'terminal' : 'outside';
	};
	const consume = (event: KeyboardEvent): void => {
		event.preventDefault();
		event.stopPropagation();
	};
	const handleKeyDown = (event: KeyboardEvent): void => {
		const focus = focusFor(event.target);
		// Copy takes the key only while it has a selection to copy; otherwise the key continues to
		// the catalog bindings and then to the program.
		if (
			matchesProjectedTerminalSelectionCopyKey(event, platform, focus, ctrlCCopiesSelection) &&
			invoke(PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.copySelection, undefined, terminalScope, {
				clearSelectionAfterCopy: true,
			})
		) {
			consume(event);
			return;
		}
		const binding = resolveProjectedTerminalShortcut(
			event,
			platform,
			focus,
			shortcutBindingSource?.current() ?? defaultProjectedTerminalShortcutBindings(platform),
		);
		if (binding === null) return;
		const scope = focus === 'find' ? findScope : terminalScope;
		const invoked = binding.nativePasteFallback
			? invoke(binding.actionId, undefined, scope, { nativePasteFallback: true })
			: invoke(binding.actionId, undefined, scope);
		if (!invoked) return;
		consume(event);
	};
	document.addEventListener('keydown', handleKeyDown, true);
	return () => {
		document.removeEventListener('keydown', handleKeyDown, true);
	};
}
