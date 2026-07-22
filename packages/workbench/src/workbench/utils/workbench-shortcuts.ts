import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';

interface WorkbenchShortcutEvent {
	readonly altKey: boolean;
	readonly code: string;
	readonly ctrlKey: boolean;
	readonly isComposing: boolean;
	readonly metaKey: boolean;
	readonly repeat: boolean;
	readonly shiftKey: boolean;
}

export const WORKBENCH_SHORTCUTS = {
	closeTab: {
		code: 'KeyW',
		id: 'workbench.tab.close',
		keyLabel: 'W',
	},
	newTerminal: {
		code: 'KeyN',
		id: 'workbench.terminal.create',
		keyLabel: 'N',
	},
	search: {
		code: 'KeyK',
		id: 'workbench.search.open',
		keyLabel: 'K',
	},
	settings: {
		code: 'Comma',
		id: 'workbench.settings.open',
		keyLabel: ',',
	},
	toggleSidebar: {
		code: 'KeyB',
		id: 'workbench.toggle-left-panel',
		keyLabel: 'B',
	},
} as const;

export type WorkbenchShortcutAction = keyof typeof WORKBENCH_SHORTCUTS;

export function formatWorkbenchShortcut(
	action: WorkbenchShortcutAction,
	platform: WorkbenchShortcutPlatform,
): string {
	const keyLabel = WORKBENCH_SHORTCUTS[action].keyLabel;
	if (platform === 'apple') return `⌘${keyLabel}`;
	return action === 'settings' ? 'Ctrl+,' : `Ctrl ${keyLabel}`;
}

export function matchesWorkbenchShortcut(
	event: WorkbenchShortcutEvent,
	platform: WorkbenchShortcutPlatform,
	action: WorkbenchShortcutAction,
): boolean {
	if (
		event.code !== WORKBENCH_SHORTCUTS[action].code ||
		event.altKey ||
		event.shiftKey ||
		event.isComposing ||
		event.repeat
	) {
		return false;
	}

	return platform === 'apple' ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}
