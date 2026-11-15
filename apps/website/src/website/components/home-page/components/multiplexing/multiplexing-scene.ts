import { timeOrdered, type Scene, type SceneFrame, type SceneStep } from '../../../../scene/scene';
import type { DesktopSidebarWorkspace } from '../../../desktop-sidebar/DesktopSidebar';
import {
	CLAUDE_HANDLE,
	CLAUDE_TITLE,
	GIT_LOG,
	OPENCODE_HANDLE,
	OUTSIDE,
	PROMPT,
	agentList,
} from '../../../session-scene/session';
import {
	green,
	plainText,
	red,
	textRow,
	type TerminalRow,
} from '../../../terminal-text/terminal-row';

// One shell, attached from the terminal interface and the desktop application at once: what the
// visitor types shows in both, and running it prints the same output in each. A terminal outside
// Orchbee reads the same shell after every command. Before the visitor types, the shell runs a
// few git commands by itself. Terminal contents are English in every page language.

/** The longest command the visitor may type. */
export const COMMAND_LENGTH = 48;
/** The shell keeps this many rows of output. */
const KEPT_ROWS = 40;
const READ_COMMAND = 'orchbee terminal read @panel:0x1f --source recent --lines 3';

/** What a known command prints. */
const OUTPUT: Readonly<Record<string, () => readonly TerminalRow[]>> = {
	'git status -sb': () => [
		textRow('## ', green('main'), '...', red('origin/main'), ' [ahead ', green('1'), ']'),
		textRow(' ', red('M'), ' src/orders/routes.ts'),
	],
	'git log --oneline -2': () => GIT_LOG,
	'git branch': () => [textRow('* ', green('main'))],
	'git diff --stat': () => [
		textRow(' src/orders/routes.ts | 14 ', green('+++++++++'), red('-----')),
		textRow(' 1 file changed, 9 insertions(+), 5 deletions(-)'),
	],
	'orchbee agent list': () =>
		agentList([
			{ handle: CLAUDE_HANDLE, agent: 'claude-code', state: 'working', cwd: 'C:\\demo\\shop-api' },
			{ handle: OPENCODE_HANDLE, agent: 'opencode', state: 'done', cwd: 'C:\\demo\\shop-web' },
		]),
	pwd: () => [
		textRow(''),
		textRow('Path'),
		textRow('----'),
		textRow('C:\\demo\\shop-api'),
		textRow(''),
	],
};

export interface MultiplexingState {
	/** What the shell printed, the commands included. */
	readonly log: readonly TerminalRow[];
	/** What is typed at the prompt. */
	readonly typed: string;
	/** Counts the commands run, so the outside terminal can show each new read. */
	readonly runs: number;
	/** The shell's rows: the log, then the prompt. */
	readonly shell: readonly TerminalRow[];
	/** The outside terminal's rows: the read command and what it printed. Changes only on a run. */
	readonly reader: readonly TerminalRow[];
}

function shellRows(log: readonly TerminalRow[], typed: string): TerminalRow[] {
	return [...log, textRow(`${PROMPT}${typed}`, { cursor: typed === '' ? 'blink' : 'steady' })];
}

// The outside terminal reads the shell's last rows, the fresh prompt included.
function readerRows(log: readonly TerminalRow[]): TerminalRow[] {
	return [
		textRow(`${OUTSIDE}${READ_COMMAND}`),
		...log.slice(-2).map((line) => textRow(plainText(line))),
		textRow(PROMPT.trimEnd()),
		textRow(OUTSIDE, { cursor: 'blink' }),
	];
}

const INITIAL_LOG: readonly TerminalRow[] = [textRow(`${PROMPT}git log --oneline -2`), ...GIT_LOG];

/** The shell after one command: what the static page and reduced motion show. */
export const MULTIPLEXING_INITIAL: MultiplexingState = {
	log: INITIAL_LOG,
	typed: '',
	runs: 0,
	shell: shellRows(INITIAL_LOG, ''),
	reader: readerRows(INITIAL_LOG),
};

/** The same state with `typed` at the prompt. */
export function typedAt(state: MultiplexingState, typed: string): MultiplexingState {
	if (state.typed === typed) return state;
	return { ...state, typed, shell: shellRows(state.log, typed) };
}

// What the shell prints for a command line, after echoing it.
function output(text: string): readonly TerminalRow[] {
	const command = text.trim().replaceAll(/\s+/g, ' ');
	const echo = /^echo\s+(.*)$/i.exec(command);
	if (echo !== null) return [textRow((echo[1] ?? '').replace(/^(["'])(.*)\1$/, '$2'))];
	const known = Object.hasOwn(OUTPUT, command) ? OUTPUT[command] : undefined;
	if (known !== undefined) return known();
	if (command === '') return [];
	const name = command.split(' ')[0] ?? '';
	return [
		textRow(
			red(
				`${name}: The term '${name}' is not recognized as a name of a cmdlet, function, script file, or executable program.`,
			),
		),
		textRow(
			red(
				'Check the spelling of the name, or if a path was included, verify that the path is correct and try again.',
			),
		),
	];
}

/** Runs `text` as a command line: echoes it, prints its output, and empties the prompt. */
export function ran(state: MultiplexingState, text: string): MultiplexingState {
	const command = text.trim().replaceAll(/\s+/g, ' ');
	const log =
		command === 'cls' || command === 'clear'
			? []
			: [...state.log, textRow(`${PROMPT}${text}`), ...output(text)].slice(-KEPT_ROWS);
	return {
		log,
		typed: '',
		runs: state.runs + 1,
		shell: shellRows(log, ''),
		reader: readerRows(log),
	};
}

/** The commands the shell runs by itself, in turn, until the visitor types. */
const SAMPLES = ['git status -sb', 'git diff --stat', 'git branch', 'git log --oneline -2'];
const FIRST_KEY_MS = 1400;
const RUN_DELAY_MS = 600;
const NEXT_SAMPLE_MS = 2800;

// The pause after the key-th key of a command: three speeds in turn.
function keyDelay(key: number): number {
	return 70 + (key % 3) * 25;
}

/**
 * The shell typing and running its sample commands, one after another. Each run carries on from
 * the last, so the commands keep adding to the same log.
 */
export function createMultiplexingScene(): Scene<MultiplexingState> {
	const steps: SceneStep<MultiplexingState>[] = [];
	let time = FIRST_KEY_MS;
	for (const sample of SAMPLES) {
		for (let key = 1; key <= sample.length; key += 1) {
			const typed = sample.slice(0, key);
			steps.push({ at: time, apply: (state) => typedAt(state, typed) });
			time += keyDelay(key);
		}
		time += RUN_DELAY_MS;
		steps.push({ at: time, apply: (state) => ran(state, sample) });
		time += NEXT_SAMPLE_MS;
	}
	// The next run's first key falls where the following sample would start.
	const length = time - FIRST_KEY_MS;
	return {
		initial: MULTIPLEXING_INITIAL,
		steps: timeOrdered(steps),
		fadeAt: length,
		length,
		carryOver: (state) => state,
	};
}

export interface MultiplexingFrame extends SceneFrame {
	readonly rows: {
		readonly shell: readonly TerminalRow[];
		readonly reader: readonly TerminalRow[];
	};
}

/** What a state shows, by binding key. */
export function multiplexingFrame(state: MultiplexingState): MultiplexingFrame {
	return {
		rows: { shell: state.shell, reader: state.reader },
		states: {},
		texts: {},
		flags: {},
	};
}

/** The sidebar both clients draw, held still: the Session's Agents at work elsewhere. */
export const MULTIPLEXING_WORKSPACES: readonly DesktopSidebarWorkspace[] = [
	{
		name: 'shop-api',
		branch: 'main',
		selected: true,
		ahead: 1,
		agents: [{ title: CLAUDE_TITLE, provider: 'Claude Code', state: 'working', unread: false }],
	},
	{
		name: 'shop-web',
		branch: 'main',
		agents: [{ title: 'Update cart layout', provider: 'OpenCode', state: 'done', unread: false }],
	},
	{ name: 'docs', branch: 'main', agents: [] },
];
