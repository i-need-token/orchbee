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

export const WORKBENCH_TOGGLE_SIDEBAR_SHORTCUT_ID = 'workbench.toggle-left-panel';

export function formatWorkbenchToggleSidebarShortcut(platform: WorkbenchShortcutPlatform): string {
	return platform === 'apple' ? '⌘ B' : 'Ctrl B';
}

export function matchesWorkbenchToggleSidebarShortcut(
	event: WorkbenchShortcutEvent,
	platform: WorkbenchShortcutPlatform,
): boolean {
	if (
		event.code !== 'KeyB' ||
		event.altKey ||
		event.shiftKey ||
		event.isComposing ||
		event.repeat
	) {
		return false;
	}

	return platform === 'apple' ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}
