import type { ActionMessageDescriptor } from '@orchbee/actions';
import { projectedTerminalMessages } from '@orchbee/i18n';
import { ActiveScreen, type ActiveScreenValue } from '@orchbee/terminal-protocol';

/** Why this Client cannot drive its terminal, in the words its surface already shows. */
type ProjectedTerminalInputBlocker = 'disconnected' | 'exited' | 'failed' | 'view-only';

/** What the surface showing a terminal knows about it, read only to say why an action cannot run. */
export interface ProjectedTerminalActionConditions {
	/** The screen the latest frame shows, or `null` before the first frame. */
	activeScreen(): ActiveScreenValue | null;
	/** The first known cause that stops input, or `null` while the surface knows none. */
	inputBlocker(): ProjectedTerminalInputBlocker | null;
}

export const UNKNOWN_PROJECTED_TERMINAL_ACTION_CONDITIONS: ProjectedTerminalActionConditions = {
	activeScreen: () => null,
	inputBlocker: () => null,
};

/** Why an action that sends input cannot run. */
export function terminalInputUnavailableReason(
	conditions: ProjectedTerminalActionConditions,
): ActionMessageDescriptor {
	switch (conditions.inputBlocker()) {
		case 'exited':
			return projectedTerminalMessages.terminalExited;
		case 'failed':
			return projectedTerminalMessages.runtimeFailure;
		case 'disconnected':
			return projectedTerminalMessages.unavailableDisconnected;
		case 'view-only':
			return projectedTerminalMessages.controlViewOnlyReason;
		case null:
			return projectedTerminalMessages.unavailableInputClosed;
	}
}

/**
 * Why a screen operation (Clear, Reset) cannot run, or `null` when nothing the surface knows stops
 * it. Another Client holding control does not: the Server runs these for any viewer.
 */
export function terminalScreenUnavailableReason(
	conditions: ProjectedTerminalActionConditions,
): ActionMessageDescriptor | null {
	const blocker = conditions.inputBlocker();
	return blocker === null || blocker === 'view-only'
		? null
		: terminalInputUnavailableReason(conditions);
}

/** Why an action that needs a drawn terminal cannot run while the terminal accepts input. */
export function terminalNotDrawnReason(
	conditions: ProjectedTerminalActionConditions,
): ActionMessageDescriptor {
	const blocker = conditions.inputBlocker();
	return blocker === null || blocker === 'view-only'
		? projectedTerminalMessages.unavailableLoading
		: terminalInputUnavailableReason(conditions);
}

/** Why an action over retained history cannot run: no frame yet, or a full-screen program. */
export function terminalHistoryUnavailableReason(
	conditions: ProjectedTerminalActionConditions,
): ActionMessageDescriptor {
	return conditions.activeScreen() === ActiveScreen.alternate
		? projectedTerminalMessages.unavailableAlternateScreen
		: terminalNotDrawnReason(conditions);
}
