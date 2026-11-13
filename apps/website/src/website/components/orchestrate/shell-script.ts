// The orchestrate loop: four orchbee commands in the shell, their real output, and the times (in
// seconds into a 20 s loop) at which each step starts. Step -1 is the empty prompt.
export const SHELL_LOOP_SECONDS = 20;

export const SHELL_MARKS = [0.7, 2.3, 3.2, 5.4, 6.2, 7.9, 10.8, 11.8, 14] as const;

export const FINAL_SHELL_STEP = SHELL_MARKS.length - 1;

export type ShellLine =
	| { readonly kind: 'command'; readonly step: number; readonly text: string }
	| {
			readonly kind: 'output';
			readonly step: number;
			readonly text: string;
			readonly tone: 'plain' | 'muted';
	  };

export const SHELL_LINES: readonly ShellLine[] = [
	{ kind: 'command', step: 0, text: 'orchbee agent start codex --split right' },
	{
		kind: 'output',
		step: 1,
		tone: 'muted',
		text: 'Started Codex in a new Panel on the right (Panel 0x36).',
	},
	{
		kind: 'output',
		step: 1,
		tone: 'muted',
		text: 'Next: orchbee agent wait --panel 0x36 --until idle',
	},
	{ kind: 'command', step: 2, text: 'orchbee agent prompt @panel:0x36 "Review feat/website"' },
	{ kind: 'output', step: 3, tone: 'muted', text: 'Prompt submitted to @panel:0x36.' },
	{ kind: 'command', step: 4, text: 'orchbee agent wait @panel:0x36 --until idle' },
	{
		kind: 'output',
		step: 5,
		tone: 'muted',
		text: '@panel:0x36 matched idle at state sequence 3 after 41210ms',
	},
	{ kind: 'command', step: 6, text: 'orchbee agent read @panel:0x36 --source recent --lines 3' },
	{ kind: 'output', step: 7, tone: 'plain', text: 'No blocking issues. Two notes:' },
	{
		kind: 'output',
		step: 7,
		tone: 'muted',
		text: '1. The copy button is skipped by Tab on macOS.',
	},
	{ kind: 'output', step: 7, tone: 'muted', text: '2. The zh-CN heading wraps badly at 320px.' },
];

/** Steps after which the shell waits at an empty prompt. */
export const PROMPT_STEPS: readonly number[] = [-1, 1, 3, 5, 8];

/** The terminal interface view each step shows: Codex absent, started, working, done, read. */
export function orchestrateViewOf(step: number): 0 | 1 | 2 | 3 | 4 {
	if (step < 1) return 0;
	if (step < 3) return 1;
	if (step < 5) return 2;
	if (step < 7) return 3;
	return 4;
}
