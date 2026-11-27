import { timeOrdered, type Scene, type SceneFrame, type SceneStep } from '../../scene/scene';
import type { AgentState } from '../state-glyph/StateGlyph';
import {
	blue,
	bold,
	cyan,
	dim,
	green,
	inputRow,
	plainText,
	row,
	textRow,
	yellow,
	type TerminalRow,
} from '../terminal-text/terminal-row';
import type { DesktopSidebarWorkspace } from '../desktop-sidebar/DesktopSidebar';

// The Session the home page plays: a shell Panel, Claude Code, Codex (started by a command), and
// OpenCode in another Workspace. The command line outside Orchbee asks the same Session and prints
// what the real commands print; the text formats are the ones in apps/orchbee/src/cli_text.
// Terminal contents are English in every page language.

const PROMPT = 'PS C:\\demo\\shop-api> ';
const OUTSIDE = 'PS C:\\demo> ';
export const CLAUDE_HANDLE = '@panel:0x2a';
export const CODEX_HANDLE = '@panel:0x36';
export const OPENCODE_HANDLE = '@panel:0x41';
const CODEX_TASK = 'Review the current changes';

/** The title of the Claude Code Agent. */
export const CLAUDE_TITLE = 'Validate order quantity';

const CLAUDE_ROWS = [
	inputRow`${dim('>')} Validate order quantity before insert`,
	row`● ${bold('Read')} ${dim('src/orders/routes.ts')}`,
	row`● ${bold('Read')} ${dim('src/orders/validate.ts')}`,
	textRow('  The quantity check runs after the insert.'),
	row`● ${bold('Update')} ${dim('src/orders/routes.ts')}`,
	row`  Allow this edit?  ${blue('❯ Yes')}   No`,
	row`● ${bold('Bash')} ${dim('npm test -- orders')}`,
	textRow('  12 passed'),
	textRow('  The check now runs before the insert.'),
] as const;
const CLAUDE_ALLOWED = row`  Allow this edit?  ${dim('Yes')}`;
export const CODEX_ROWS = [
	inputRow`${dim('›')} `,
	inputRow`${dim('›')} ${CODEX_TASK}`,
	row`• ${bold('Ran')} ${dim('git diff --stat')}`,
	textRow('  3 files changed, 41 insertions(+), 9 deletions(-)'),
	row`• ${bold('Read')} ${dim('src/orders/routes.ts')}`,
	row`• ${bold('Read')} ${dim('test/orders.test.ts')}`,
	textRow('  One test still posts quantity 0 on purpose.'),
	textRow('  Suggest updating the fixture in orders.test.ts.'),
] as const;
const GIT_LOG = [
	row`${yellow('2905180')} (${cyan('HEAD ->')} ${green('main')}) Add order routes`,
	row`${yellow('a6baeab')} Initial service skeleton`,
] as const;
const BLANK = textRow('');

/** The state words `orchbee agent list` prints. */
const CLI_STATE: Readonly<Record<AgentState, string>> = {
	working: 'Working',
	attention: 'Blocked',
	done: 'Done',
	idle: 'Idle',
	failed: 'Error',
};

type AgentId = 'cc' | 'cx' | 'oc';

/** The Agents of the Session; the status line counts at most this many. */
export const SESSION_AGENTS: readonly AgentId[] = ['cc', 'cx', 'oc'];

interface Shell {
	readonly prompt: string;
	readonly log: readonly TerminalRow[];
	readonly typed: string;
	readonly typing: boolean;
	/** A command is running, so no prompt is shown. */
	readonly waiting: boolean;
	/** What the terminal shows: the log, then the prompt. Kept with the state so it changes only
	 * when the shell does. */
	readonly rows: readonly TerminalRow[];
}

export interface SessionState {
	readonly shell: Shell;
	readonly cli: Shell;
	readonly claude: readonly TerminalRow[];
	readonly codex: readonly TerminalRow[];
	readonly states: Readonly<Record<AgentId, AgentState>>;
	readonly unread: Readonly<Record<'cc' | 'cx', boolean>>;
	readonly codexStarted: boolean;
	readonly codexTitle: string;
	/** Counts Codex's state changes, as `orchbee agent wait` reports them. */
	readonly sequence: number;
}

type ShellName = 'shell' | 'cli';
type ShellChange = Partial<Pick<Shell, 'log' | 'typed' | 'typing' | 'waiting'>>;

function withRows(shell: Omit<Shell, 'rows'>): Shell {
	const cursor = { cursor: shell.typing ? 'steady' : 'blink' } as const;
	return {
		...shell,
		rows: shell.waiting
			? shell.log
			: [...shell.log, textRow(`${shell.prompt}${shell.typed}`, cursor)],
	};
}

function changeShell(
	state: SessionState,
	name: ShellName,
	change: (shell: Shell) => ShellChange,
): SessionState {
	const shell = state[name];
	return { ...state, [name]: withRows({ ...shell, ...change(shell) }) };
}

function printed(state: SessionState, name: ShellName, rows: readonly TerminalRow[]): SessionState {
	return changeShell(state, name, (shell) => ({ log: [...shell.log, ...rows] }));
}

function said(state: SessionState, agent: 'claude' | 'codex', line: TerminalRow): SessionState {
	return agent === 'claude'
		? { ...state, claude: [...state.claude, line] }
		: { ...state, codex: [...state.codex, line] };
}

function withState(state: SessionState, agent: AgentId, next: AgentState): SessionState {
	const sequence = agent === 'cx' && state.states.cx !== next ? state.sequence + 1 : state.sequence;
	return { ...state, states: { ...state.states, [agent]: next }, sequence };
}

function withUnread(state: SessionState, agent: 'cc' | 'cx', unread: boolean): SessionState {
	return { ...state, unread: { ...state.unread, [agent]: unread } };
}

/** An Agent as `orchbee agent list` prints it. */
export interface AgentListEntry {
	readonly handle: string;
	readonly agent: string;
	readonly state: AgentState;
	readonly cwd: string;
}

/** `orchbee agent list`, default text: six padded columns, two spaces apart. */
export function agentListLines(agents: readonly AgentListEntry[]): string[] {
	const widths = [4, Math.max(...agents.map((agent) => agent.handle.length)), 12, 4, 9];
	const line = (cells: readonly string[]): string =>
		cells.map((cell, index) => cell.padEnd(widths[index] ?? 0)).join('  ');
	return [
		line(['#', 'HANDLE', 'AGENT', 'NAME', 'STATE', 'CWD']),
		...agents.map((agent, index) =>
			line([String(index + 1), agent.handle, agent.agent, '-', CLI_STATE[agent.state], agent.cwd]),
		),
	];
}

/** The same table as terminal rows. */
export function agentList(agents: readonly AgentListEntry[]): TerminalRow[] {
	return agentListLines(agents).map((line) => textRow(line));
}

function agentTable(state: SessionState): TerminalRow[] {
	return agentList([
		{
			handle: CLAUDE_HANDLE,
			agent: 'claude-code',
			state: state.states.cc,
			cwd: 'C:\\demo\\shop-api',
		},
		...(state.codexStarted
			? [
					{
						handle: CODEX_HANDLE,
						agent: 'codex',
						state: state.states.cx,
						cwd: 'C:\\demo\\shop-api',
					},
				]
			: []),
		{
			handle: OPENCODE_HANDLE,
			agent: 'opencode',
			state: state.states.oc,
			cwd: 'C:\\demo\\shop-web',
		},
	]);
}

function initialState(): SessionState {
	const state: SessionState = {
		shell: withRows({
			prompt: PROMPT,
			log: [textRow(`${PROMPT}git log --oneline -2`), ...GIT_LOG, BLANK],
			typed: '',
			typing: false,
			waiting: false,
		}),
		cli: withRows({ prompt: OUTSIDE, log: [], typed: '', typing: false, waiting: false }),
		claude: [CLAUDE_ROWS[0], CLAUDE_ROWS[1]],
		codex: [],
		states: { cc: 'working', cx: 'idle', oc: 'done' },
		unread: { cc: false, cx: false },
		codexStarted: false,
		codexTitle: 'Codex',
		sequence: 1,
	};
	return printed(state, 'cli', [
		textRow(`${OUTSIDE}orchbee agent list`),
		...agentTable(state),
		BLANK,
	]);
}

/** What `orchbee agent read --source recent --lines 3` prints: the last rows as plain text. */
function recent(rows: readonly TerminalRow[]): TerminalRow[] {
	return rows.slice(-3).map((line) => textRow(plainText(line)));
}

/** The scripted Session: who types what and when, and what changes. */
export function createSessionScene(): Scene<SessionState> {
	const steps: SceneStep<SessionState>[] = [];
	const at = (time: number, apply: (state: SessionState) => SessionState): void => {
		steps.push({ at: time, apply });
	};
	// Typing speed varies from key to key, the same way on every run.
	let seed = 11;
	const random = (): number => {
		seed = (seed * 16_807) % 2_147_483_647;
		return seed / 2_147_483_647;
	};
	const type = (name: ShellName, start: number, text: string): number => {
		let time = start;
		at(time, (state) => changeShell(state, name, () => ({ typing: true })));
		for (const character of text) {
			time += 22 + random() * 28 + (character === ' ' ? 20 : 0);
			at(time, (state) =>
				changeShell(state, name, (shell) => ({ typed: shell.typed + character })),
			);
		}
		return time + 320;
	};
	const enter = (
		name: ShellName,
		time: number,
		then: (state: SessionState) => SessionState,
	): void => {
		at(time, (state) =>
			then(
				changeShell(state, name, (shell) => ({
					log: [...shell.log, textRow(`${shell.prompt}${shell.typed}`)],
					typed: '',
					typing: false,
				})),
			),
		);
	};
	const say = (time: number, agent: 'claude' | 'codex', line: TerminalRow): void => {
		at(time, (state) => said(state, agent, line));
	};
	const listing = (state: SessionState): SessionState =>
		printed(state, 'cli', [...agentTable(state), BLANK]);

	say(700, 'claude', CLAUDE_ROWS[2]);
	let time = type('shell', 1200, 'orchbee agent start codex --split down');
	enter('shell', time, (state) => ({
		...printed(state, 'shell', [
			textRow(`Started Codex in a new Panel below (Panel ${CODEX_HANDLE}).`),
			textRow(`Next: orchbee agent wait --panel ${CODEX_HANDLE} --until idle`),
		]),
		codex: [CODEX_ROWS[0]],
		codexStarted: true,
	}));
	say(time + 800, 'claude', CLAUDE_ROWS[3]);

	time = type('shell', time + 1300, `orchbee agent prompt ${CODEX_HANDLE} "${CODEX_TASK}"`);
	enter('shell', time, (state) =>
		withState(
			{
				...printed(state, 'shell', [textRow(`Prompt submitted to ${CODEX_HANDLE}.`)]),
				codex: [CODEX_ROWS[1]],
				codexTitle: CODEX_TASK,
			},
			'cx',
			'working',
		),
	);
	say(time + 300, 'claude', CLAUDE_ROWS[4]);
	say(time + 700, 'codex', CODEX_ROWS[2]);
	at(time + 1000, (state) =>
		withUnread(withState(said(state, 'claude', CLAUDE_ROWS[5]), 'cc', 'attention'), 'cc', true),
	);
	say(time + 1300, 'codex', CODEX_ROWS[3]);
	enter('cli', type('cli', time + 1400, 'orchbee agent list'), listing);

	time = type('shell', time + 1600, `orchbee agent wait ${CODEX_HANDLE} --until idle`);
	enter('shell', time, (state) => changeShell(state, 'shell', () => ({ waiting: true })));
	say(time - 900, 'codex', CODEX_ROWS[4]);
	at(time + 500, (state) =>
		withUnread(
			withState(
				{ ...state, claude: [...state.claude.slice(0, -1), CLAUDE_ALLOWED] },
				'cc',
				'working',
			),
			'cc',
			false,
		),
	);
	say(time + 900, 'claude', CLAUDE_ROWS[6]);
	say(time + 700, 'codex', CODEX_ROWS[5]);
	say(time + 1700, 'codex', CODEX_ROWS[6]);
	say(time + 2500, 'codex', CODEX_ROWS[7]);
	at(time + 3000, (state) => {
		const idle = withUnread(withState(state, 'cx', 'idle'), 'cx', true);
		return printed(
			changeShell(idle, 'shell', () => ({ waiting: false })),
			'shell',
			[textRow(`${CODEX_HANDLE} matched idle at state sequence ${idle.sequence} after 3014ms`)],
		);
	});

	time = type('shell', time + 3500, `orchbee agent read ${CODEX_HANDLE} --source recent --lines 3`);
	enter('shell', time, (state) => printed(state, 'shell', recent(state.codex)));
	say(time - 1500, 'claude', CLAUDE_ROWS[7]);
	say(time - 500, 'claude', CLAUDE_ROWS[8]);
	at(time + 300, (state) => withUnread(withState(state, 'cc', 'done'), 'cc', true));
	const last = type(
		'cli',
		time + 900,
		`orchbee agent read ${CLAUDE_HANDLE} --source recent --lines 3`,
	);
	enter('cli', last, (state) => printed(state, 'cli', [...recent(state.claude), BLANK]));
	enter('cli', type('cli', last + 1500, 'orchbee agent list'), listing);

	return {
		initial: initialState(),
		steps: timeOrdered(steps),
		fadeAt: last + 8600,
		length: last + 8960,
	};
}

/** The order the status line names states in; Agents in other states are not counted. */
export const SUMMARY_STATES = ['attention', 'working', 'done', 'idle'] as const;
export type SummaryState = (typeof SUMMARY_STATES)[number];

/** The page-language words a frame needs, read from the page by script. */
export interface SessionStrings {
	readonly state: (state: AgentState) => string;
	readonly summary: (state: SummaryState, count: number) => string;
}

export function stateStringKey(state: AgentState): string {
	return `state:${state}`;
}

export function summaryStringKey(state: SummaryState, count: number): string {
	return `summary:${state}:${count}`;
}

/** The lookups a frame is made with, from the page's strings by key. */
export function sessionStrings(lookup: (key: string) => string | undefined): SessionStrings {
	return {
		state: (state) => lookup(stateStringKey(state)) ?? '',
		summary: (state, count) => lookup(summaryStringKey(state, count)) ?? '',
	};
}

export interface SessionFrame extends SceneFrame {
	readonly rows: {
		readonly shell: readonly TerminalRow[];
		readonly cli: readonly TerminalRow[];
		readonly claude: readonly TerminalRow[];
		readonly codex: readonly TerminalRow[];
	};
	readonly states: {
		readonly cc: AgentState;
		readonly cx: AgentState;
		readonly oc: AgentState;
		/** The Tab shows the most urgent state of its Agents. */
		readonly tab: AgentState;
		/** The status line shows the first state it names. */
		readonly summary: AgentState;
	};
	readonly texts: {
		readonly 'title:cx': string;
		readonly 'word:cc': string;
		readonly 'word:cx': string;
		readonly 'word:oc': string;
		readonly summary: string;
	};
	readonly flags: {
		readonly 'started:cx': boolean;
		readonly 'unread:cc': boolean;
		readonly 'unread:cx': boolean;
		readonly 'unread:any': boolean;
	};
}

/** What a state of the Session shows, by binding key. */
export function sessionFrame(state: SessionState, strings: SessionStrings): SessionFrame {
	const { states } = state;
	const counted = state.codexStarted ? SESSION_AGENTS : SESSION_AGENTS.filter((id) => id !== 'cx');
	const count = (kind: SummaryState): number => counted.filter((id) => states[id] === kind).length;
	const present = SUMMARY_STATES.filter((kind) => count(kind) > 0);
	const tab = SUMMARY_STATES.find(
		(kind) => states.cc === kind || (state.codexStarted && states.cx === kind),
	);
	return {
		rows: {
			shell: state.shell.rows,
			cli: state.cli.rows,
			claude: state.claude,
			codex: state.codex,
		},
		states: { ...states, tab: tab ?? 'idle', summary: present[0] ?? 'idle' },
		texts: {
			'title:cx': state.codexTitle,
			'word:cc': strings.state(states.cc),
			'word:cx': strings.state(states.cx),
			'word:oc': strings.state(states.oc),
			summary: present.map((kind) => strings.summary(kind, count(kind))).join(' · '),
		},
		flags: {
			'started:cx': state.codexStarted,
			'unread:cc': state.unread.cc,
			'unread:cx': state.unread.cx,
			'unread:any': state.unread.cc || state.unread.cx,
		},
	};
}

/** An Agent of the Session as the windows list it, with the binding keys of its values. */
export interface SessionAgent {
	readonly id: AgentId;
	readonly title: string;
	readonly provider: string;
	readonly state: AgentState;
	/** The state in the page language. */
	readonly word: string;
	readonly unread: boolean;
	readonly shown: boolean;
	readonly bind: {
		readonly state: string;
		readonly word: string;
		readonly unread?: string;
		readonly title?: string;
		readonly shown?: string;
	};
}

/** The Agents of the Session in list order; Codex is shown once it has started. */
export function sessionAgents(
	frame: SessionFrame,
): readonly [SessionAgent, SessionAgent, SessionAgent] {
	const { states, texts, flags } = frame;
	return [
		{
			id: 'cc',
			title: CLAUDE_TITLE,
			provider: 'Claude Code',
			state: states.cc,
			word: texts['word:cc'],
			unread: flags['unread:cc'],
			shown: true,
			bind: { state: 'cc', word: 'word:cc', unread: 'unread:cc' },
		},
		{
			id: 'cx',
			title: texts['title:cx'],
			provider: 'Codex',
			state: states.cx,
			word: texts['word:cx'],
			unread: flags['unread:cx'],
			shown: flags['started:cx'],
			bind: {
				state: 'cx',
				word: 'word:cx',
				unread: 'unread:cx',
				title: 'title:cx',
				shown: 'started:cx',
			},
		},
		{
			id: 'oc',
			title: 'Update cart layout',
			provider: 'OpenCode',
			state: states.oc,
			word: texts['word:oc'],
			unread: false,
			shown: true,
			bind: { state: 'oc', word: 'word:oc' },
		},
	];
}

/** The sidebar list of the Session: Claude Code and Codex in one Workspace, OpenCode in another.
 * The shop-api branch is one commit ahead of its upstream, which the desktop sidebar shows. */
export function sessionWorkspaces(frame: SessionFrame): readonly DesktopSidebarWorkspace[] {
	const [claude, codex, openCode] = sessionAgents(frame);
	return [
		{ name: 'shop-api', branch: 'main', selected: true, ahead: 1, agents: [claude, codex] },
		{ name: 'shop-web', branch: 'main', agents: [openCode] },
		{ name: 'docs', branch: 'main', agents: [] },
	];
}
