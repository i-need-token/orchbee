import { expect, it } from 'vitest';
import type {
	WorkbenchAgentBoardRow,
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
} from '../../../workbench';
import type { AgentSessionSummary } from '../../server-connection';
import {
	projectServerAgentBoardRows,
	type ServerAgentBoardSidebarAgent,
} from '../projectServerAgentBoardRows';

it('uses the same completion fact for the board, fences replacement generations and preserves provider blockers', () => {
	const id = '2'.repeat(32);
	const generation = '1'.repeat(32);
	const summary: AgentSessionSummary = {
		sessionId: 'session',
		providerId: 'codex',
		parentSessionId: null,
		lifecycle: 'live',
		executionLocation: { executionLocationId: 'local' },
		pendingRequestCount: 0,
		observation: {
			source: 'terminal',
			fidelity: 'heuristic',
			freshness: 'fresh',
			generation: 1,
			sequence: 1,
		},
		presentationAttachments: [
			{ presentationId: id, generation, navigable: true, visibility: 'visible' },
		],
		status: { kind: 'idle' },
	};
	const fact: WorkbenchAgentObservation = {
		agent: 'codex',
		binding: generation + ':epoch:terminal',
		sequence: 1n,
		state: 'idle',
		completion: 'awaiting-input',
	};
	const snapshot = (current: WorkbenchAgentObservation): WorkbenchAgentSnapshot => ({
		complete: true,
		presentations: new Map([[id, current]]),
	});
	expect(projectServerAgentBoardRows([summary], snapshot(fact), [])[0]?.status.kind).toBe(
		'succeeded',
	);
	expect(
		projectServerAgentBoardRows(
			[{ ...summary, status: { kind: 'succeeded' } }],
			snapshot({ ...fact, state: 'done', completion: 'acknowledged' }),
			[],
		)[0]?.status.kind,
	).toBe('idle');
	expect(
		projectServerAgentBoardRows(
			[summary],
			snapshot({ ...fact, binding: 'replacement:epoch:terminal' }),
			[],
		)[0]?.status.kind,
	).toBe('idle');
	expect(
		projectServerAgentBoardRows(
			[{ ...summary, status: { kind: 'needs-attention', count: 1 } }],
			snapshot(fact),
			[],
		)[0]?.status,
	).toEqual({ kind: 'needs-attention', count: 1 });
});

function session(
	sessionId: string,
	presentationId: string,
	status: AgentSessionSummary['status'] = { activity: null, kind: 'working', phase: 'working' },
): AgentSessionSummary {
	return {
		executionLocation: { executionLocationId: 'local' },
		lifecycle: 'live',
		observation: {
			fidelity: 'rich',
			freshness: 'fresh',
			generation: 1,
			sequence: 1,
			source: 'hook',
		},
		parentSessionId: null,
		pendingRequestCount: 0,
		presentationAttachments: [
			{ generation: '1'.repeat(32), navigable: true, presentationId, visibility: 'visible' },
		],
		providerId: 'claude-code',
		sessionId,
		status,
	};
}

type WorkbenchAgentState = WorkbenchAgentObservation['state'];

function sidebar(presentationId: string, state: WorkbenchAgentState): ServerAgentBoardSidebarAgent {
	return {
		observation: {
			agent: 'claude',
			binding: `${presentationId}:epoch:terminal`,
			sequence: 1n,
			state,
		},
		presentationId,
	};
}

const keys = (rows: readonly WorkbenchAgentBoardRow[]): readonly string[] =>
	rows.map((row) => row.sessionId);

it.each<{
	readonly expected: readonly string[];
	readonly name: string;
	readonly sessions: readonly AgentSessionSummary[];
	readonly sidebarAgents: readonly ServerAgentBoardSidebarAgent[];
}>([
	{ expected: ['a'], name: 'canonical only', sessions: [session('a', 'p1')], sidebarAgents: [] },
	{
		expected: ['terminal:p1', 'terminal:p2'],
		name: 'sidebar only, in the sidebar order',
		sessions: [],
		sidebarAgents: [sidebar('p1', 'working'), sidebar('p2', 'idle')],
	},
	{
		expected: ['a'],
		name: 'a session attaching the sidebar Agent replaces its row',
		sessions: [session('a', 'p1')],
		sidebarAgents: [sidebar('p1', 'blocked')],
	},
	{
		expected: ['b', 'terminal:p4', 'a', 'terminal:p3'],
		name: 'sessions then sidebar Agents, needing attention lifted first',
		sessions: [session('a', 'p1'), session('b', 'p2', { count: 2, kind: 'needs-attention' })],
		sidebarAgents: [sidebar('p3', 'idle'), sidebar('p4', 'blocked')],
	},
])('lists $name', ({ expected, sessions, sidebarAgents }) => {
	expect(keys(projectServerAgentBoardRows(sessions, undefined, sidebarAgents))).toEqual(expected);
});

it('drops the row of an Agent that leaves the sidebar', () => {
	const sessions = [session('a', 'p1')];
	expect(
		keys(projectServerAgentBoardRows(sessions, undefined, [sidebar('p2', 'working')])),
	).toEqual(['a', 'terminal:p2']);
	expect(keys(projectServerAgentBoardRows(sessions, undefined, []))).toEqual(['a']);
});

it('maps the sidebar state onto the board status as the TUI does', () => {
	const states: readonly WorkbenchAgentState[] = [
		'working',
		'blocked',
		'done',
		'error',
		'idle',
		'unknown',
		'opening',
	];
	const rows = projectServerAgentBoardRows(
		[],
		undefined,
		states.map((state, index) => sidebar(`p${index}`, state)),
	);
	expect(rows.map((row) => row.status)).toEqual([
		{ count: 1, kind: 'needs-attention' },
		{ kind: 'working' },
		{ kind: 'succeeded' },
		{ kind: 'failed' },
		{ kind: 'idle' },
		{ kind: 'unknown' },
		{ kind: 'unknown' },
	]);
	expect(rows[0]).toMatchObject({
		origin: 'sidebar',
		presentationId: 'p1',
		providerId: 'claude-code',
	});
});
