import type { ActionAvailability } from '@orchbee/actions';
import { projectedTerminalMessages } from '@orchbee/i18n';

/** Why this window cannot drive its terminal, in the words its surface already shows. */
type ProjectedTerminalInputBlocker = 'disconnected' | 'exited' | 'failed' | 'view-only';

/** What the surface showing a terminal knows about it, read only to say why an action cannot run. */
export interface ProjectedTerminalActionConditions {
	/** The first known cause that stops input, or `null` while the surface knows none. */
	inputBlocker(): ProjectedTerminalInputBlocker | null;
}

export const UNKNOWN_PROJECTED_TERMINAL_ACTION_CONDITIONS: ProjectedTerminalActionConditions = {
	inputBlocker: () => null,
};

/**
 * A terminal action that cannot run. It names a reason only when the fix is not obvious, which for a
 * terminal is a lost connection; every other cause leaves the row simply grey.
 */
export function terminalActionDisabled(
	conditions: ProjectedTerminalActionConditions,
): ActionAvailability {
	return conditions.inputBlocker() === 'disconnected'
		? { kind: 'disabled', reason: projectedTerminalMessages.unavailableDisconnected }
		: { kind: 'disabled' };
}

/**
 * Whether something the surface knows stops a screen operation (Clear, Reset). Another window typing
 * does not: the Server runs these for any viewer.
 */
export function terminalScreenBlocked(conditions: ProjectedTerminalActionConditions): boolean {
	const blocker = conditions.inputBlocker();
	return blocker !== null && blocker !== 'view-only';
}
