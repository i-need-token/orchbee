import type { ActionScope } from '@orchbee/actions';

import {
	defaultProjectedTerminalShortcutBindings,
	resolveProjectedTerminalShortcut,
	type ProjectedTerminalShortcutBinding,
	type ProjectedTerminalShortcutBindingSource,
	type ProjectedTerminalShortcutFocus,
	type ProjectedTerminalShortcutPlatform,
} from '../../projected-terminal-keyboard-shortcuts';

export interface InstallProjectedTerminalShortcutsOptions {
	readonly document: Document;
	readonly findRoot: HTMLElement;
	readonly invoke: (
		actionId: ProjectedTerminalShortcutBinding['actionId'],
		input: undefined,
		scope: ActionScope,
	) => boolean;
	readonly findScope: ActionScope;
	readonly platform: ProjectedTerminalShortcutPlatform;
	readonly shortcutBindingSource?: ProjectedTerminalShortcutBindingSource | undefined;
	readonly terminalRoot: HTMLElement;
	readonly terminalScope: ActionScope;
}

export function installProjectedTerminalShortcuts({
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
	const handleKeyDown = (event: KeyboardEvent): void => {
		const focus = focusFor(event.target);
		const binding = resolveProjectedTerminalShortcut(
			event,
			platform,
			focus,
			shortcutBindingSource?.current() ?? defaultProjectedTerminalShortcutBindings(platform),
		);
		if (binding === null) return;
		const scope = focus === 'find' ? findScope : terminalScope;
		if (!invoke(binding.actionId, undefined, scope)) return;
		event.preventDefault();
		event.stopPropagation();
	};
	document.addEventListener('keydown', handleKeyDown, true);
	return () => {
		document.removeEventListener('keydown', handleKeyDown, true);
	};
}
