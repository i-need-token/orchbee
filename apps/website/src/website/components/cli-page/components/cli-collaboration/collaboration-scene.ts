import { timeOrdered, type Scene, type SceneFrame, type SceneStep } from '../../../../scene/scene';
import { CLAUDE_HANDLE, CODEX_HANDLE, CODEX_ROWS, agentList } from '../../../session-scene/session';
import type { AgentState } from '../../../state-glyph/StateGlyph';
import {
	bold,
	dim,
	inputRow,
	plainText,
	row,
	textRow,
	type TerminalRow,
} from '../../../terminal-text/terminal-row';

// Claude Code directs Codex with five `orchbee` commands, run through its shell tool: it starts
// Codex in a Panel beside its own, finds its handle, prompts it, waits for it to finish, and reads
// what it found. Each step lasts a fixed time, so any step can be shown or replayed from the steps
// before it. Terminal contents are English in every page language.

const CODEX_TASK = 'Review the current changes';

/** The command each step runs, in order. */
export const COLLABORATION_COMMANDS = [
	'orchbee agent start codex --split right',
	'orchbee agent list',
	`orchbee agent prompt ${CODEX_HANDLE} "${CODEX_TASK}"`,
	`orchbee agent wait ${CODEX_HANDLE} --until idle --timeout 120000`,
	`orchbee agent read ${CODEX_HANDLE} --source recent --lines 40`,
] as const;

export interface CollaborationState {
	readonly claude: readonly TerminalRow[];
	readonly codex: readonly TerminalRow[];
	/** Codex's Panel is open beside Claude Code's. */
	readonly split: boolean;
	readonly codexState: AgentState;
	readonly codexTitle: string;
	/** The step under way or last finished; -1 before the first. */
	readonly step: number;
	readonly running: boolean;
}

const INITIAL: CollaborationState = {
	claude: [inputRow`${dim('>')} Ask Codex to review the current changes`],
	codex: [],
	split: false,
	codexState: 'idle',
	codexTitle: 'Codex',
	step: -1,
	running: false,
};

type Change = (state: CollaborationState) => CollaborationState;

function claudeSays(...lines: readonly TerminalRow[]): Change {
	return (state) => ({ ...state, claude: [...state.claude, ...lines] });
}

function codexSays(line: TerminalRow): Change {
	return (state) => ({ ...state, codex: [...state.codex, line] });
}

function bash(command: string): TerminalRow {
	return row`● ${bold('Bash')} ${dim(command)}`;
}

// What the tool printed, indented under its call.
function printed(lines: readonly string[]): TerminalRow[] {
	return lines.map((line) => textRow(`  ${line}`));
}

const [CODEX_EMPTY, CODEX_PROMPTED, ...CODEX_WORK] = CODEX_ROWS;

interface Step {
	readonly duration: number;
	/** Changes at milliseconds after the step starts. */
	readonly changes: readonly (readonly [number, Change])[];
}

const STEPS: readonly Step[] = [
	{
		duration: 3600,
		changes: [
			[0, claudeSays(bash(COLLABORATION_COMMANDS[0]))],
			[
				900,
				(state) => ({
					...claudeSays(
						...printed([
							`Started Codex in a new Panel on the right (Panel ${CODEX_HANDLE}).`,
							`Next: orchbee agent wait --panel ${CODEX_HANDLE} --until idle`,
						]),
					)(state),
					split: true,
					codex: [CODEX_EMPTY],
				}),
			],
		],
	},
	{
		duration: 3200,
		changes: [
			[0, claudeSays(bash(COLLABORATION_COMMANDS[1]))],
			[
				800,
				claudeSays(
					...printed(
						agentList([
							{
								handle: CLAUDE_HANDLE,
								agent: 'claude-code',
								state: 'working',
								cwd: 'C:\\demo\\shop-api',
							},
							{ handle: CODEX_HANDLE, agent: 'codex', state: 'idle', cwd: 'C:\\demo\\shop-api' },
						]).map(plainText),
					),
				),
			],
		],
	},
	{
		duration: 3800,
		changes: [
			[0, claudeSays(bash(COLLABORATION_COMMANDS[2]))],
			[
				800,
				(state) => ({
					...claudeSays(...printed([`Prompt submitted to ${CODEX_HANDLE}.`]))(state),
					codex: [CODEX_PROMPTED],
					codexTitle: CODEX_TASK,
					codexState: 'working',
				}),
			],
			[1800, codexSays(CODEX_WORK[0])],
			[2600, codexSays(CODEX_WORK[1])],
		],
	},
	{
		duration: 5200,
		changes: [
			[0, claudeSays(bash(COLLABORATION_COMMANDS[3]))],
			[900, codexSays(CODEX_WORK[2])],
			[1900, codexSays(CODEX_WORK[3])],
			[2900, codexSays(CODEX_WORK[4])],
			[3700, codexSays(CODEX_WORK[5])],
			[
				4300,
				(state) => ({
					...claudeSays(
						...printed([`${CODEX_HANDLE} matched idle at state sequence 3 after 4318ms`]),
					)(state),
					codexState: 'idle',
				}),
			],
		],
	},
	{
		duration: 5600,
		changes: [
			[0, claudeSays(bash(COLLABORATION_COMMANDS[4]))],
			[900, (state) => claudeSays(...printed(state.codex.map(plainText)))(state)],
			[2600, claudeSays(textRow('● Codex found one test that still posts quantity 0.'))],
		],
	},
];

/** How long the finished steps stay on screen before the first step plays again. */
const REST_MS = 5200;

/** When each step starts, and when the last one ends. */
const STARTS = STEPS.reduce<number[]>(
	(starts, step, index) => [...starts, (starts[index] ?? 0) + step.duration],
	[0],
);

/** When step `step` starts, in milliseconds after the scene starts. */
export function stepStart(step: number): number {
	return STARTS[step] ?? 0;
}

/** The step under way `at` milliseconds after the scene starts. */
export function stepAt(at: number): number {
	const next = STARTS.findIndex((start) => start > at);
	return next === -1 ? STEPS.length - 1 : Math.min(Math.max(next - 1, 0), STEPS.length - 1);
}

/** The steps in order: each marks itself under way, then makes its changes. */
export function createCollaborationScene(): Scene<CollaborationState> {
	const steps: SceneStep<CollaborationState>[] = [];
	STEPS.forEach((step, index) => {
		const start = stepStart(index);
		steps.push({ at: start, apply: (state) => ({ ...state, step: index, running: true }) });
		for (const [delay, change] of step.changes) steps.push({ at: start + delay, apply: change });
	});
	const end = stepStart(STEPS.length);
	steps.push({ at: end, apply: (state) => ({ ...state, running: false }) });
	const length = end + REST_MS;
	return { initial: INITIAL, steps: timeOrdered(steps), fadeAt: length, length };
}

/** The state once steps 0 to `step` have finished: what choosing a step shows when nothing plays. */
export function collaborationAfter(step: number): CollaborationState {
	const scene = createCollaborationScene();
	const end = stepStart(step + 1);
	const state = scene.steps
		.filter((sceneStep) => sceneStep.at < end)
		.reduce((current, sceneStep) => sceneStep.apply(current), scene.initial);
	return { ...state, step, running: false };
}

export interface CollaborationFrame extends SceneFrame {
	readonly rows: {
		readonly claude: readonly TerminalRow[];
		readonly codex: readonly TerminalRow[];
	};
	readonly states: { readonly codex: AgentState };
	readonly texts: { readonly 'title:codex': string };
	readonly flags: { readonly split: boolean };
}

/** What a state shows in the window, by binding key. The step rail is the activation's. */
export function collaborationFrame(state: CollaborationState): CollaborationFrame {
	return {
		rows: { claude: state.claude, codex: state.codex },
		states: { codex: state.codexState },
		texts: { 'title:codex': state.codexTitle },
		flags: { split: state.split },
	};
}

/** How the rail shows step `index`: under way, finished, or still to come. */
export function stepProgress(state: CollaborationState, index: number): 'on' | 'done' | 'next' {
	if (state.running && index === state.step) return 'on';
	if (index < state.step || (!state.running && index === state.step)) return 'done';
	return 'next';
}

/** How long step `index` lasts, for the rail's progress line. */
export function stepDuration(index: number): number {
	return STEPS[index]?.duration ?? 0;
}
