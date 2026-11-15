import type { AgentState } from '../../../state-glyph/StateGlyph';

/** The Agents of the ring. */
export type RingAgent = 'claude' | 'codex' | 'opencode' | 'gemini' | 'pi';

/**
 * A cell of the ring: an Agent, its state, and the lines its terminal prints in turn. `shown` is
 * how many lines it shows at first, which is also what the page shows without script. `pause`
 * holds it in another state for a while after it prints the line at that index, as a question
 * waits for a person or a finished turn shows until the next one starts. The module imports no
 * catalog, so the ring's activation stays small.
 */
export interface RingCell {
	readonly id: RingAgent;
	readonly state: AgentState;
	readonly lines: readonly string[];
	readonly shown: number;
	readonly pause?: { readonly after: number; readonly state: AgentState; readonly ms: number };
}

/** The most lines a cell keeps; older lines leave at the top. */
export const RING_LINES = 6;

/** How often one of the working Agents prints its next line. */
export const RING_STEP_MS = 640;

const CLAUDE_LINES = [
	'● Read routes.ts',
	'● Read validate.ts',
	'  check runs after insert',
	'● Update routes.ts',
	'● Bash npm test',
	'  12 passed',
];
const CODEX_LINES = [
	'• Ran git diff --stat',
	'  3 files changed',
	'• Read routes.ts',
	'• Read orders.test.ts',
	'  one test posts 0',
	'  suggest: fix fixture',
];
const OPENCODE_LINES = [
	'~ Edit CartLayout.tsx',
	'~ Run pnpm build',
	'  built in 4.1s',
	'  Cart layout updated',
];
const GEMINI_LINES = [
	'✦ Read README.md',
	'✦ Read docs/install.md',
	'✦ Draft install guide',
	'  Overwrite install.md?',
	'✦ Write install.md',
	'  2 files written',
];
const PI_LINES = [
	'$ rg "quantity" src',
	'  9 matches',
	'$ edit validate.ts',
	'$ npm run lint',
	'  no problems',
];

/** The five cells, clockwise from the top. Terminal lines are English in every language. */
export const RING_CELLS: readonly RingCell[] = [
	{
		id: 'claude',
		state: 'working',
		lines: CLAUDE_LINES,
		shown: 3,
		pause: { after: CLAUDE_LINES.length - 1, state: 'done', ms: 5200 },
	},
	{
		id: 'codex',
		state: 'working',
		lines: CODEX_LINES,
		shown: 3,
		pause: { after: CODEX_LINES.length - 1, state: 'idle', ms: 3600 },
	},
	{
		id: 'opencode',
		state: 'done',
		lines: OPENCODE_LINES,
		shown: OPENCODE_LINES.length,
	},
	{
		id: 'gemini',
		state: 'working',
		lines: GEMINI_LINES,
		shown: 3,
		pause: { after: 3, state: 'attention', ms: 4200 },
	},
	{
		id: 'pi',
		state: 'working',
		lines: PI_LINES,
		shown: 3,
	},
];
