import type { TerminalPresentation } from '../components/workbench-panel-workspace/model/panel-layout-types';

/** What a close does: ends the terminals still running, and discards the output of the ended ones. */
export interface TerminalCloseImpact {
	readonly ended: number;
	readonly running: number;
}

/**
 * Counts as the TUI does: a terminal ended once its launch failed or its Host settled it where this
 * Client saw. Every other terminal, however idle, still runs, and so does a Presentation the Server
 * bound no terminal to, since nothing proves it ended.
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
		case 'exited':
		case 'failed':
		case 'launch-failed':
			return true;
		case 'running':
		case 'unbound':
			return false;
	}
}
