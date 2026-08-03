import type { ActionScope } from '@orchbee/actions';

import { PROJECTED_TERMINAL_ACTION_IDS } from './projected-terminal-action-ids';

export type ProjectedTerminalShortcutPlatform = 'apple' | 'standard';
type ProjectedTerminalShortcutFocus = 'find' | 'outside' | 'terminal';

interface ProjectedTerminalShortcutEvent {
	readonly altKey: boolean;
	readonly code: string;
	readonly ctrlKey: boolean;
	readonly isComposing: boolean;
	readonly metaKey: boolean;
	readonly shiftKey: boolean;
}

type ProjectedTerminalShortcutBinding =
	| { readonly actionId: 'terminal.search.open'; readonly input: undefined }
	| { readonly actionId: 'terminal.selection.copy'; readonly input: undefined }
	| { readonly actionId: 'terminal.selection.select-all'; readonly input: undefined };

export interface InstallProjectedTerminalShortcutsOptions {
	readonly document: Document;
	readonly findRoot: HTMLElement;
	readonly invoke: (
		actionId: ProjectedTerminalShortcutBinding['actionId'],
		input: undefined,
		scope: ActionScope,
	) => void;
	readonly findScope: ActionScope;
	readonly platform: ProjectedTerminalShortcutPlatform;
	readonly terminalRoot: HTMLElement;
	readonly terminalScope: ActionScope;
}

function resolveProjectedTerminalShortcut(
	event: ProjectedTerminalShortcutEvent,
	platform: ProjectedTerminalShortcutPlatform,
	focus: ProjectedTerminalShortcutFocus,
): ProjectedTerminalShortcutBinding | null {
	if (focus === 'outside' || event.altKey || event.isComposing) return null;
	const primary =
		platform === 'apple' ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
	if (primary && !event.shiftKey && event.code === 'KeyF') {
		return { actionId: PROJECTED_TERMINAL_ACTION_IDS.findOpen, input: undefined };
	}
	if (focus !== 'terminal') return null;
	if (platform === 'apple' && event.metaKey && !event.ctrlKey && !event.shiftKey) {
		if (event.code === 'KeyC') {
			return { actionId: PROJECTED_TERMINAL_ACTION_IDS.copySelection, input: undefined };
		}
		if (event.code === 'KeyA') {
			return { actionId: PROJECTED_TERMINAL_ACTION_IDS.selectAll, input: undefined };
		}
	}
	if (
		platform === 'standard' &&
		event.ctrlKey &&
		!event.metaKey &&
		event.shiftKey &&
		event.code === 'KeyC'
	) {
		return { actionId: PROJECTED_TERMINAL_ACTION_IDS.copySelection, input: undefined };
	}
	return null;
}

export function installProjectedTerminalShortcuts({
	document,
	findRoot,
	findScope,
	invoke,
	platform,
	terminalRoot,
	terminalScope,
}: InstallProjectedTerminalShortcutsOptions): () => void {
	const consumedCodes = new Set<string>();
	const focusFor = (target: EventTarget | null): ProjectedTerminalShortcutFocus => {
		const NodeConstructor = findRoot.ownerDocument.defaultView?.Node;
		if (NodeConstructor === undefined || !(target instanceof NodeConstructor)) return 'outside';
		if (findRoot.contains(target)) return 'find';
		return terminalRoot.contains(target) ? 'terminal' : 'outside';
	};
	const handleKeyDown = (event: KeyboardEvent): void => {
		const binding = resolveProjectedTerminalShortcut(event, platform, focusFor(event.target));
		if (binding === null) return;
		if (!event.repeat) {
			const scope = focusFor(event.target) === 'find' ? findScope : terminalScope;
			invoke(binding.actionId, binding.input, scope);
		}
		consumedCodes.add(event.code);
		event.preventDefault();
		event.stopImmediatePropagation();
	};
	const handleKeyUp = (event: KeyboardEvent): void => {
		if (!consumedCodes.delete(event.code)) return;
		event.preventDefault();
		event.stopImmediatePropagation();
	};
	document.addEventListener('keydown', handleKeyDown, true);
	document.addEventListener('keyup', handleKeyUp, true);
	return () => {
		document.removeEventListener('keydown', handleKeyDown, true);
		document.removeEventListener('keyup', handleKeyUp, true);
	};
}
