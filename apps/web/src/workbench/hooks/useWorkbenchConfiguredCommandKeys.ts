import { useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import { formatWorkbenchConfiguredCommandName } from '../model/workbench-configured-command-name';
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

/** The configured commands the window follows, which of its keys run them, and what they took. */
export interface WorkbenchConfiguredCommandKeysView {
	readonly catalog: readonly WorkbenchConfiguredCommand[];
	readonly keys: WorkbenchConfiguredCommandKeys;
	/** The name of the command holding each default it took, by the claim's id, as Search names it. */
	readonly takenBy: ReadonlyMap<string, string>;
}

const NO_COMMANDS: readonly WorkbenchConfiguredCommand[] = [];

/**
 * Follows the configured commands, so a reloaded configuration binds its keys again. The window and
 * its shortcut settings read the one result, so both say the same about a chord a command took.
 */
export function useWorkbenchConfiguredCommandKeys({
	claims,
	commands,
	platform,
}: UseWorkbenchConfiguredCommandKeysOptions): WorkbenchConfiguredCommandKeysView {
	const intl = useOrchbeeIntl();
	const readCatalog = () => commands?.getSnapshot() ?? NO_COMMANDS;
	const catalog = React.useSyncExternalStore(
		commands?.subscribe ?? subscribeToNothing,
		readCatalog,
		readCatalog,
	);
	return React.useMemo(() => {
		const keys = resolveWorkbenchConfiguredCommandKeys(catalog, claims, platform);
		const takenBy = new Map(
			Array.from(keys.taken, ([claimId, commandId]) => [
				claimId,
				formatWorkbenchConfiguredCommandName(intl, catalog, commandId),
			]),
		);
		return { catalog, keys, takenBy };
	}, [catalog, claims, intl, platform]);
}
