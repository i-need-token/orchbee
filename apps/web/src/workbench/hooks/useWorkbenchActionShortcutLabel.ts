import { useMemoizedFn } from '@orchbee/foundation/react';

import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import type { WorkbenchTerminalActionShortcutLabels } from '../types/workbench-terminal-action-shortcut-labels';
import {
	formatWorkbenchActionShortcut,
	type WorkbenchShortcutBindings,
} from '../utils/workbench-shortcuts';

interface UseWorkbenchActionShortcutLabelOptions {
	readonly shortcutBindings: WorkbenchShortcutBindings;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly terminalActionShortcutLabels: WorkbenchTerminalActionShortcutLabels | undefined;
}

export function useWorkbenchActionShortcutLabel({
	shortcutBindings,
	shortcutPlatform,
	terminalActionShortcutLabels,
}: UseWorkbenchActionShortcutLabelOptions): (actionId: string) => string | undefined {
	return useMemoizedFn((actionId: string): string | undefined => {
		const workbenchShortcut = formatWorkbenchActionShortcut(
			actionId,
			shortcutPlatform,
			shortcutBindings,
		);
		if (workbenchShortcut !== null) return workbenchShortcut;
		return Object.entries(terminalActionShortcutLabels ?? {}).find(
			([terminalActionId]) => terminalActionId === actionId,
		)?.[1];
	});
}
