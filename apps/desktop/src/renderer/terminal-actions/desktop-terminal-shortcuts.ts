import type { ActionScope } from '@orchbee/actions';

import { DESKTOP_TERMINAL_ACTION_IDS } from './desktop-terminal-action-ids';

export type DesktopShortcutPlatform = 'apple' | 'standard';
type DesktopTerminalShortcutFocus = 'find' | 'outside' | 'terminal';

interface DesktopTerminalShortcutEvent {
	readonly altKey: boolean;
	readonly code: string;
	readonly ctrlKey: boolean;
	readonly isComposing: boolean;
	readonly metaKey: boolean;
	readonly shiftKey: boolean;
}

type DesktopTerminalShortcutBinding =
	| { readonly actionId: 'terminal.search.open'; readonly input: undefined }
	| { readonly actionId: 'terminal.selection.copy'; readonly input: undefined }
	| { readonly actionId: 'terminal.selection.select-all'; readonly input: undefined };

export interface InstallDesktopTerminalShortcutsOptions {
	readonly document: Document;
	readonly findRoot: HTMLElement;
	readonly invoke: (
		actionId: DesktopTerminalShortcutBinding['actionId'],
		input: undefined,
		scope: ActionScope,
	) => void;
	readonly findScope: ActionScope;
	readonly platform: DesktopShortcutPlatform;
	readonly terminalRoot: HTMLElement;
	readonly terminalScope: ActionScope;
}

function resolveDesktopTerminalShortcut(
	event: DesktopTerminalShortcutEvent,
	platform: DesktopShortcutPlatform,
	focus: DesktopTerminalShortcutFocus,
): DesktopTerminalShortcutBinding | null {
	if (focus === 'outside' || event.altKey || event.isComposing) return null;
	const primary =
		platform === 'apple' ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
	if (primary && !event.shiftKey && event.code === 'KeyF') {
		return { actionId: DESKTOP_TERMINAL_ACTION_IDS.findOpen, input: undefined };
	}
	if (focus !== 'terminal') return null;
	if (platform === 'apple' && event.metaKey && !event.ctrlKey && !event.shiftKey) {
		if (event.code === 'KeyC') {
			return { actionId: DESKTOP_TERMINAL_ACTION_IDS.copySelection, input: undefined };
		}
		if (event.code === 'KeyA') {
			return { actionId: DESKTOP_TERMINAL_ACTION_IDS.selectAll, input: undefined };
		}
	}
	if (
		platform === 'standard' &&
		event.ctrlKey &&
		!event.metaKey &&
		event.shiftKey &&
		event.code === 'KeyC'
	) {
		return { actionId: DESKTOP_TERMINAL_ACTION_IDS.copySelection, input: undefined };
	}
	return null;
}

export function installDesktopTerminalShortcuts({
	document,
	findRoot,
	findScope,
	invoke,
	platform,
	terminalRoot,
	terminalScope,
}: InstallDesktopTerminalShortcutsOptions): () => void {
	const consumedCodes = new Set<string>();
	const focusFor = (target: EventTarget | null): DesktopTerminalShortcutFocus => {
		const NodeConstructor = findRoot.ownerDocument.defaultView?.Node;
		if (NodeConstructor === undefined || !(target instanceof NodeConstructor)) return 'outside';
		if (findRoot.contains(target)) return 'find';
		return terminalRoot.contains(target) ? 'terminal' : 'outside';
	};
	const handleKeyDown = (event: KeyboardEvent): void => {
		const binding = resolveDesktopTerminalShortcut(event, platform, focusFor(event.target));
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

export function resolveDesktopShortcutPlatform(platform: string): DesktopShortcutPlatform {
	return /Mac|iPhone|iPad|iPod/u.test(platform) ? 'apple' : 'standard';
}
