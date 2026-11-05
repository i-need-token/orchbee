import { useMemoizedFn } from '@orchbee/foundation/react';
import { formatKeyboardShortcutChord } from '@orchbee/keyboard-shortcuts';

import { workbenchConfiguredCommandIdOf } from '../actions/workbench-configured-command-action-id';
import type { WorkbenchConfiguredCommandKeys } from '../model/workbench-configured-command-keys';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import type { WorkbenchTerminalActionShortcutLabels } from '../types/workbench-terminal-action-shortcut-labels';
import {
	formatWorkbenchActionShortcut,
	type WorkbenchShortcutBindings,
} from '../utils/workbench-shortcuts';

interface UseWorkbenchActionShortcutLabelOptions {
	readonly configuredCommandKeys: WorkbenchConfiguredCommandKeys;
	readonly shortcutBindings: WorkbenchShortcutBindings;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly terminalActionShortcutLabels: WorkbenchTerminalActionShortcutLabels | undefined;
}

/**
 * The chord that runs an Action now: a configured command's first bound chord, and no label for a
 * default whose chord a configured command took.
 */
export function useWorkbenchActionShortcutLabel({
	configuredCommandKeys,
	shortcutBindings,
	shortcutPlatform,
	terminalActionShortcutLabels,
}: UseWorkbenchActionShortcutLabelOptions): (actionId: string) => string | undefined {
	return useMemoizedFn((actionId: string): string | undefined => {
		const commandId = workbenchConfiguredCommandIdOf(actionId);
		if (commandId !== null) {
			const chord = configuredCommandKeys.chords.get(commandId)?.[0];
			return chord === undefined ? undefined : formatKeyboardShortcutChord(chord, shortcutPlatform);
		}
		const label =
			formatWorkbenchActionShortcut(actionId, shortcutPlatform, shortcutBindings) ??
			Object.entries(terminalActionShortcutLabels ?? {}).find(
				([terminalActionId]) => terminalActionId === actionId,
			)?.[1];
		return label === undefined || configuredCommandKeys.heldLabels.has(label) ? undefined : label;
	});
}
