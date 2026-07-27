import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionId } from '../actions/workbench-action-ids';

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
	newTerminal: {
		code: 'KeyN',
		id: WORKBENCH_ACTION_IDS.createTerminal,
		keyLabel: 'N',
	},
	search: {
		code: 'KeyK',
		id: WORKBENCH_ACTION_IDS.openSearch,
		keyLabel: 'K',
	},
	settings: {
		code: 'Comma',
		id: WORKBENCH_ACTION_IDS.openSettings,
		keyLabel: ',',
	},
	toggleSidebar: {
		code: 'KeyB',
		id: WORKBENCH_ACTION_IDS.toggleSidebar,
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

export function resolveWorkbenchShortcut(
	event: WorkbenchShortcutEvent,
	platform: WorkbenchShortcutPlatform,
): WorkbenchActionId | null {
	for (const binding of Object.values(WORKBENCH_SHORTCUTS)) {
		if (matchesBinding(event, platform, binding)) {
			return binding.id;
		}
	}
	return null;
}

function matchesBinding(
	event: WorkbenchShortcutEvent,
	platform: WorkbenchShortcutPlatform,
	binding: Readonly<{ code: string }>,
): boolean {
	if (
		event.code !== binding.code ||
		event.altKey ||
		event.shiftKey ||
		event.isComposing ||
		event.repeat
	) {
		return false;
	}

	return platform === 'apple' ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}
