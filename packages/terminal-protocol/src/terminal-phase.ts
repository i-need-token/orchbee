// Wire vocabulary only. Terminal lifecycle transitions and invariants are
// owned by apps/terminal-host; protocol consumers use this vocabulary for
// the phase carried by a terminal inventory summary.
export const TerminalPhase = {
	awaitingStart: 'awaiting-start',
	containingFailure: 'containing-failure',
	exited: 'exited',
	failed: 'failed',
	finishing: 'finishing',
	running: 'running',
} as const;

export type TerminalPhase = (typeof TerminalPhase)[keyof typeof TerminalPhase];
