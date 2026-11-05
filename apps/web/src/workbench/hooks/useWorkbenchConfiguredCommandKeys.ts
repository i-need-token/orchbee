import * as React from 'react';

import {
	resolveWorkbenchConfiguredCommandKeys,
	type WorkbenchConfiguredCommandKeys,
} from '../model/workbench-configured-command-keys';
import type {
	WorkbenchConfiguredCommand,
	WorkbenchConfiguredCommands,
} from '../types/workbench-configured-commands';
import type { WorkbenchShortcutClaim } from '../types/workbench-shortcut-claims';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import { subscribeToNothing } from '../utils/subscribeToNothing';

interface UseWorkbenchConfiguredCommandKeysOptions {
	readonly claims: readonly WorkbenchShortcutClaim[];
	readonly commands: WorkbenchConfiguredCommands | undefined;
	readonly platform: WorkbenchShortcutPlatform;
}

interface WorkbenchConfiguredCommandKeysView {
	readonly catalog: readonly WorkbenchConfiguredCommand[];
	readonly keys: WorkbenchConfiguredCommandKeys;
}

const NO_COMMANDS: readonly WorkbenchConfiguredCommand[] = [];

/** Follows the configured commands, so a reloaded configuration binds its keys again. */
export function useWorkbenchConfiguredCommandKeys({
	claims,
	commands,
	platform,
}: UseWorkbenchConfiguredCommandKeysOptions): WorkbenchConfiguredCommandKeysView {
	const readCatalog = () => commands?.getSnapshot() ?? NO_COMMANDS;
	const catalog = React.useSyncExternalStore(
		commands?.subscribe ?? subscribeToNothing,
		readCatalog,
		readCatalog,
	);
	const keys = React.useMemo(
		() => resolveWorkbenchConfiguredCommandKeys(catalog, claims, platform),
		[catalog, claims, platform],
	);
	return { catalog, keys };
}
