import { z } from 'zod';

// Wire vocabulary only. Terminal lifecycle transitions and invariants are
// owned by apps/terminal-host; protocol consumers use this schema solely to
// validate the phase carried by a terminal inventory summary.
export const TerminalPhase = {
	containingFailure: 'containing-failure',
	exited: 'exited',
	failed: 'failed',
	finishing: 'finishing',
	running: 'running',
} as const;

export const TerminalPhaseSchema = z.enum(TerminalPhase);

export type TerminalPhase = (typeof TerminalPhase)[keyof typeof TerminalPhase];
