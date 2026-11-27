import { useMemoizedFn } from '@orchbee/foundation/react';
import {
	formatKeyboardShortcutChord,
	formatKeyboardShortcutSequence,
} from '@orchbee/keyboard-shortcuts';

import { workbenchConfiguredCommandIdOf } from '../actions/workbench-configured-command-action-id';
import type { WorkbenchConfiguredCommandKeys } from '../model/workbench-configured-command-keys';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import type { WorkbenchTerminalActionShortcutLabels } from '../types/workbench-terminal-action-shortcut-labels';
import {
	formatWorkbenchActionShortcut,
	type WorkbenchScopedShortcutBinding,
	type WorkbenchShortcutBindings,
} from '../utils/workbench-shortcuts';

interface UseWorkbenchActionShortcutLabelOptions {
	readonly configuredCommandKeys: WorkbenchConfiguredCommandKeys;
	/** The File Bar family's keys, where the File Bar is offered. */
	readonly scopedShortcutBindings?: readonly WorkbenchScopedShortcutBinding[] | undefined;
	readonly shortcutBindings: WorkbenchShortcutBindings;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly terminalActionShortcutLabels: WorkbenchTerminalActionShortcutLabels | undefined;
}

/**
 * The chord that runs an Action now: a configured command's first bound chord, and no label for a
 * default whose chord a configured command took. A File Bar family Action shows its window-wide
 * keys, else the keys of the region that owns them.
 */
export function useWorkbenchActionShortcutLabel({
	configuredCommandKeys,
	scopedShortcutBindings,
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
		if (label !== undefined) {
			return configuredCommandKeys.heldLabels.has(label) ? undefined : label;
		}
		const scoped = (scopedShortcutBindings ?? []).filter(
			(binding) => binding.definition.actionId === actionId,
		);
		const windowWide = scoped.find((binding) => binding.definition.scope === 'shell');
		if (windowWide !== undefined) {
			// A configured command on the same keys runs instead everywhere a window key reaches.
			const windowLabel = formatKeyboardShortcutSequence(windowWide.sequence, shortcutPlatform);
			return configuredCommandKeys.heldLabels.has(windowLabel) ? undefined : windowLabel;
		}
		const regional = scoped[0];
		return regional === undefined
			? undefined
			: formatKeyboardShortcutSequence(regional.sequence, shortcutPlatform);
	});
}
