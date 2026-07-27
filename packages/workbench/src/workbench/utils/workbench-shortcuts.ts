import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import { WORKBENCH_ACTION_IDS } from '../actions/workbench-action-ids';

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
	actionsSearch: {
		code: 'KeyP',
		id: WORKBENCH_ACTION_IDS.openSearch,
		input: { filter: 'actions' },
		keyLabel: 'P',
		shiftKey: true,
	},
	newTerminal: {
		code: 'KeyN',
		id: WORKBENCH_ACTION_IDS.createTerminal,
		input: { groupId: null },
		keyLabel: 'N',
		shiftKey: false,
	},
	search: {
		code: 'KeyK',
		id: WORKBENCH_ACTION_IDS.openSearch,
		input: { filter: 'all' },
		keyLabel: 'K',
		shiftKey: false,
	},
	settings: {
		code: 'Comma',
		id: WORKBENCH_ACTION_IDS.openSettings,
		input: undefined,
		keyLabel: ',',
		shiftKey: false,
	},
	toggleSidebar: {
		code: 'KeyB',
		id: WORKBENCH_ACTION_IDS.toggleSidebar,
		input: undefined,
		keyLabel: 'B',
		shiftKey: false,
	},
} as const;

export type WorkbenchShortcutAction = keyof typeof WORKBENCH_SHORTCUTS;
export type WorkbenchShortcutBinding =
	(typeof WORKBENCH_SHORTCUTS)[keyof typeof WORKBENCH_SHORTCUTS];

export function formatWorkbenchShortcut(
	action: WorkbenchShortcutAction,
	platform: WorkbenchShortcutPlatform,
): string {
	let shiftLabel = '';
	if (WORKBENCH_SHORTCUTS[action].shiftKey) {
		shiftLabel = platform === 'apple' ? '⇧' : ' Shift';
	}
	const keyLabel = WORKBENCH_SHORTCUTS[action].keyLabel;
	if (platform === 'apple') return `⌘${shiftLabel}${keyLabel}`;
	return action === 'settings' ? 'Ctrl+,' : `Ctrl${shiftLabel} ${keyLabel}`;
}

export function formatWorkbenchActionShortcut(
	actionId: string,
	platform: WorkbenchShortcutPlatform,
): string | null {
	switch (actionId) {
		case WORKBENCH_ACTION_IDS.createTerminal:
			return formatWorkbenchShortcut('newTerminal', platform);
		case WORKBENCH_ACTION_IDS.openSearch:
			return formatWorkbenchShortcut('search', platform);
		case WORKBENCH_ACTION_IDS.openSettings:
			return formatWorkbenchShortcut('settings', platform);
		case WORKBENCH_ACTION_IDS.toggleSidebar:
			return formatWorkbenchShortcut('toggleSidebar', platform);
		default:
			return null;
	}
}

export function resolveWorkbenchShortcut(
	event: WorkbenchShortcutEvent,
	platform: WorkbenchShortcutPlatform,
): WorkbenchShortcutBinding | null {
	for (const binding of Object.values(WORKBENCH_SHORTCUTS)) {
		if (matchesBinding(event, platform, binding)) {
			return binding;
		}
	}
	return null;
}

function matchesBinding(
	event: WorkbenchShortcutEvent,
	platform: WorkbenchShortcutPlatform,
	binding: Readonly<{ code: string; shiftKey: boolean }>,
): boolean {
	if (
		event.code !== binding.code ||
		event.altKey ||
		event.shiftKey !== binding.shiftKey ||
		event.isComposing ||
		event.repeat
	) {
		return false;
	}

	return platform === 'apple' ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}
