import type { TerminalPresentation } from '../components/workbench-panel-workspace/model/panel-layout-types';

/** What a close does: ends the terminals still running, and discards the output of the ended ones. */
export interface TerminalCloseImpact {
	readonly ended: number;
	readonly running: number;
}

/**
 * Counts as the TUI does: a terminal ended once its launch failed, its Host settled it where this
 * Client saw, or its disposal is proven. Every other terminal, however idle, still runs.
 */
export function terminalCloseImpact(
	terminals: readonly TerminalPresentation[],
): TerminalCloseImpact {
	const ended = terminals.filter(terminalHasEnded).length;
	return { ended, running: terminals.length - ended };
}

/** The values the close warning reads: which of its sentences applies, and both counts. */
export function terminalCloseWarningValues(impact: TerminalCloseImpact): {
	readonly ended: number;
	readonly impact: 'ended' | 'mixed' | 'none' | 'running';
	readonly running: number;
} {
	return { ended: impact.ended, impact: closeImpactKind(impact), running: impact.running };
}

function closeImpactKind({ ended, running }: TerminalCloseImpact) {
	if (running === 0) return ended === 0 ? 'none' : 'ended';
	return ended === 0 ? 'running' : 'mixed';
}

function terminalHasEnded({ lifecycle }: TerminalPresentation): boolean {
	switch (lifecycle.kind) {
		case 'closed':
		case 'exited':
		case 'failed':
		case 'provision-failed':
		case 'restore-failed':
		case 'stopped':
			return true;
		case 'close-failed':
		case 'closing':
		case 'preview':
		case 'provisioning':
		case 'reconciling':
		case 'restoring':
		case 'running':
		case 'stop-failed':
		case 'stopping':
			return false;
	}
}
