import type { WorkbenchWorkspaceLocation } from '@orchbee/workbench';
import { parseWorkbenchResponse, type WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { createServerWorkspaceLocations } from '../createServerWorkspaceLocations';
import type { ServerWorkbenchAnswer } from '../ServerWorkbenchSession';
import {
	acceptWorkspaceGit,
	followWorkspaceSnapshot,
	NO_WORKSPACE_LOCATIONS,
	presentWorkspaceLocations,
	withdrawWorkspaceGit,
	type WorkspaceLocationState,
} from '../workspace-locations';

/** The TUI's `SPACE_HOLD_BOUND`. */
const HOLD_MILLISECONDS = 1_000;
const MAIN = { branch: 'main', divergence: { ahead: 2, behind: 0 } };
const FEATURE = { branch: 'feature', divergence: null };

function hex(value: number): string {
	return value.toString(16).padStart(32, '0');
}

interface Shape {
	readonly label?: string;
	readonly terminalDirectory?: string;
}

/** One Workspace in `C:\work\one`, named One unless `shape` says otherwise. */
function snapshot(revision: number, shape: Shape = {}): WorkbenchSnapshot {
	const response = parseWorkbenchResponse({
		kind: 'workbench.snapshot.result',
		outcome: {
			kind: 'available',
			snapshot: {
				kind: 'workbench.snapshot',
				navigation: {
					directories: [{ directoryId: hex(0x301), path: 'C:\\work\\one', workerId: hex(0x2) }],
					projects: [],
					workspaces: [
						{
							label: shape.label ?? 'One',
							primaryDirectoryId: hex(0x301),
							projectId: null,
							terminalContextId: hex(0x501),
							workspaceId: hex(0x401),
							...(shape.terminalDirectory === undefined
								? {}
								: { terminalDirectory: shape.terminalDirectory }),
						},
					],
				},
				protocolVersion: 1,
				revision: `${revision}`,
				sessionId: hex(0x1),
				terminalContexts: [{ groups: [], tabs: [], terminalContextId: hex(0x501) }],
			},
		},
		protocolVersion: 1,
		requestId: hex(0xf00),
	});
	if (response.kind !== 'workbench.snapshot.result' || response.outcome.kind !== 'available') {
		throw new Error('The fixture is not an available snapshot.');
	}
	return response.outcome.snapshot;
}

function workspaceIdOf(value: WorkbenchSnapshot) {
	const workspace = value.navigation.workspaces.at(0);
	if (workspace === undefined) throw new Error('The fixture has no Workspace.');
	return workspace.workspaceId;
}

const WORKSPACE_ID = workspaceIdOf(snapshot(1));

function row(state: WorkspaceLocationState, now = 0): WorkbenchWorkspaceLocation | undefined {
	return presentWorkspaceLocations(state, now, null).get(WORKSPACE_ID);
}

/** The Workspace at revision 1, where Git reported `main`. */
function settledOnMain(): WorkspaceLocationState {
	return acceptWorkspaceGit(
		followWorkspaceSnapshot(NO_WORKSPACE_LOCATIONS, snapshot(1), 0),
		WORKSPACE_ID,
		{ git: MAIN, kind: 'observed', revision: '1' },
		'complete',
	);
}

// The TUI's carrying rules (`checkpoint.rs`), so a moved Workspace changes once in every Client.
describe('Workspace locations', () => {
	it('keeps facts while the source stays and takes a directory’s last answer when it returns', () => {
		let state = followWorkspaceSnapshot(settledOnMain(), snapshot(2), 0);
		expect(row(state)?.git).toEqual(MAIN);
		state = followWorkspaceSnapshot(state, snapshot(3, { terminalDirectory: 'C:\\work\\two' }), 0);
		state = acceptWorkspaceGit(
			state,
			WORKSPACE_ID,
			{ git: FEATURE, kind: 'observed', revision: '3' },
			'partial',
		);
		state = followWorkspaceSnapshot(state, snapshot(4), 0);
		expect(row(state)).toEqual({ folder: 'C:\\work\\one', git: MAIN, heldName: null });
	});

	it('holds what a moved row settled on until Git answers, at most a second from the first move', () => {
		let state = followWorkspaceSnapshot(
			settledOnMain(),
			snapshot(2, { label: 'Two', terminalDirectory: 'C:\\work\\two' }),
			100,
		);
		const held = { folder: 'C:\\work\\one', git: MAIN, heldName: 'One' };
		expect(row(state, 100)).toEqual(held);
		state = followWorkspaceSnapshot(
			state,
			snapshot(3, { label: 'Two', terminalDirectory: 'C:\\work\\three' }),
			600,
		);
		expect(row(state, 600)).toEqual(held);
		expect(row(state, 100 + HOLD_MILLISECONDS)).toEqual({
			folder: 'C:\\work\\three',
			git: null,
			heldName: null,
		});
		state = acceptWorkspaceGit(
			state,
			WORKSPACE_ID,
			{ git: FEATURE, kind: 'observed', revision: '3' },
			'partial',
		);
		expect(row(state, 600)).toEqual({ folder: 'C:\\work\\three', git: FEATURE, heldName: null });
	});

	it('ends a hold on a rename or a complete round without an answer, never on a partial read', () => {
		const moved = followWorkspaceSnapshot(
			settledOnMain(),
			snapshot(2, { terminalDirectory: 'C:\\work\\two' }),
			0,
		);
		const unanswered = { kind: 'unanswered' } as const;
		expect(row(acceptWorkspaceGit(moved, WORKSPACE_ID, unanswered, 'partial'))?.git).toEqual(MAIN);
		expect(row(acceptWorkspaceGit(moved, WORKSPACE_ID, unanswered, 'complete'))?.git).toBeNull();
		const renamed = followWorkspaceSnapshot(
			moved,
			snapshot(3, { label: 'Renamed', terminalDirectory: 'C:\\work\\two' }),
			0,
		);
		expect(row(renamed)).toEqual({ folder: 'C:\\work\\two', git: null, heldName: null });
	});

	it('takes facts only at the revision it follows, and forgets them all on withdrawal', () => {
		const state = followWorkspaceSnapshot(NO_WORKSPACE_LOCATIONS, snapshot(5), 0);
		expect(
			acceptWorkspaceGit(
				state,
				WORKSPACE_ID,
				{ git: MAIN, kind: 'observed', revision: '4' },
				'complete',
			),
		).toBe(state);
		const withdrawn = followWorkspaceSnapshot(
			followWorkspaceSnapshot(
				withdrawWorkspaceGit(settledOnMain()),
				snapshot(2, { terminalDirectory: 'C:\\work\\two' }),
				0,
			),
			snapshot(3),
			0,
		);
		expect(row(withdrawn, HOLD_MILLISECONDS)).toEqual({
			folder: 'C:\\work\\one',
			git: null,
			heldName: null,
		});
	});

	it('shortens a home folder as the TUI does', () => {
		const folder = (path: string, home: string) =>
			presentWorkspaceLocations(
				followWorkspaceSnapshot(
					NO_WORKSPACE_LOCATIONS,
					snapshot(1, { terminalDirectory: path }),
					0,
				),
				0,
				home,
			).get(WORKSPACE_ID)?.folder;
		const cases = [
			['C:\\Users\\Someone', 'C:\\Users\\someone\\'],
			['c:/users/someone/code/orchbee/', 'C:\\Users\\Someone'],
			['C:\\Users\\Someone2\\code', 'C:\\Users\\Someone'],
			['/home/me/src', '/home/me/'],
			['/srv', '/'],
		] as const;
		expect(cases.map(([path, home]) => folder(path, home))).toEqual([
			'~',
			'~/code/orchbee',
			'C:\\Users\\Someone2\\code',
			'~/src',
			'/srv',
		]);
	});
});

describe('createServerWorkspaceLocations', () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	it('asks Git only while watched, and reads a moved Workspace before the next round', async () => {
		vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'] });
		let current = snapshot(1);
		const listeners = new Set<(next: WorkbenchSnapshot) => void>();
		const asked: string[] = [];
		const locations = createServerWorkspaceLocations({
			homeDirectory: 'C:\\work',
			session: {
				async ask(request): Promise<ServerWorkbenchAnswer> {
					if (request.kind !== 'workbench.workspace-git') return { kind: 'not-submitted' };
					asked.push(current.revision);
					return {
						kind: 'answered',
						response: parseWorkbenchResponse({
							kind: 'workbench.workspace-git.result',
							outcome: {
								branch: 'main',
								divergence: null,
								kind: 'observed',
								workbenchRevision: current.revision,
							},
							protocolVersion: 1,
							requestId: request.requestId,
							sessionId: request.sessionId,
							workspaceId: request.workspaceId,
						}),
					};
				},
				get snapshot() {
					return current;
				},
				subscribe(listener) {
					listeners.add(listener);
					return () => listeners.delete(listener);
				},
			},
		});
		await vi.advanceTimersByTimeAsync(10_000);
		expect(asked).toEqual([]);

		const stop = locations.subscribe(() => undefined);
		await vi.advanceTimersByTimeAsync(0);
		expect(asked).toEqual(['1']);
		expect(locations.getSnapshot().get(WORKSPACE_ID)).toEqual({
			folder: '~\\one',
			git: { branch: 'main', divergence: null },
			heldName: null,
		});

		current = snapshot(2, { terminalDirectory: 'C:\\work\\two' });
		for (const listener of listeners) listener(current);
		await vi.advanceTimersByTimeAsync(0);
		expect(asked).toEqual(['1', '2']);

		stop();
		await vi.advanceTimersByTimeAsync(60_000);
		expect(asked).toEqual(['1', '2']);
		locations.dispose();
	});
});
