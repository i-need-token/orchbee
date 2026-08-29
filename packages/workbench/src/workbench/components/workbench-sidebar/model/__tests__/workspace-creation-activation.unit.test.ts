import { describe, expect, it } from 'vitest';

import {
	INITIAL_WORKSPACE_CREATION_ACTIVATION_STATE,
	transitionWorkspaceCreationActivation,
} from '../workspace-creation-activation';

const target = { projectId: 'project-1', workspaceId: 'workspace-2' } as const;

describe('transitionWorkspaceCreationActivation', () => {
	it('waits for the exact committed Workspace before activating it once', () => {
		const mounted = transitionWorkspaceCreationActivation(
			INITIAL_WORKSPACE_CREATION_ACTIVATION_STATE,
			{ kind: 'mount' },
		).state;
		const waiting = transitionWorkspaceCreationActivation(mounted, {
			kind: 'request',
			target,
		});
		expect(waiting).toEqual({ commands: [], state: { kind: 'waiting', target } });

		const unrelated = transitionWorkspaceCreationActivation(waiting.state, {
			kind: 'projection-observed',
			targets: [
				{ projectId: 'project-2', workspaceId: target.workspaceId },
				{ projectId: target.projectId, workspaceId: 'workspace-3' },
			],
		});
		expect(unrelated).toEqual(waiting);

		const activated = transitionWorkspaceCreationActivation(unrelated.state, {
			kind: 'projection-observed',
			targets: [target],
		});
		expect(activated).toEqual({
			commands: [{ kind: 'activate-workspace', target }],
			state: { kind: 'idle' },
		});
		expect(
			transitionWorkspaceCreationActivation(activated.state, {
				kind: 'projection-observed',
				targets: [target],
			}),
		).toEqual({ commands: [], state: { kind: 'idle' } });
	});

	it('activates when publication was already visible before the successful result', () => {
		const mounted = transitionWorkspaceCreationActivation(
			INITIAL_WORKSPACE_CREATION_ACTIVATION_STATE,
			{ kind: 'mount' },
		).state;
		const waiting = transitionWorkspaceCreationActivation(mounted, {
			kind: 'request',
			target,
		}).state;

		expect(
			transitionWorkspaceCreationActivation(waiting, {
				kind: 'projection-observed',
				targets: [target],
			}),
		).toEqual({
			commands: [{ kind: 'activate-workspace', target }],
			state: { kind: 'idle' },
		});
	});

	it('keeps repeated identities unchanged and lets the latest distinct request win', () => {
		const mounted = transitionWorkspaceCreationActivation(
			INITIAL_WORKSPACE_CREATION_ACTIVATION_STATE,
			{ kind: 'mount' },
		).state;
		const first = transitionWorkspaceCreationActivation(mounted, {
			kind: 'request',
			target,
		});
		expect(transitionWorkspaceCreationActivation(first.state, { kind: 'request', target })).toEqual(
			first,
		);

		const latestTarget = { projectId: 'project-1', workspaceId: 'workspace-3' } as const;
		expect(
			transitionWorkspaceCreationActivation(first.state, {
				kind: 'request',
				target: latestTarget,
			}),
		).toEqual({ commands: [], state: { kind: 'waiting', target: latestTarget } });
	});

	it('suppresses late requests after disposal and starts a fresh mounted lifetime', () => {
		const mounted = transitionWorkspaceCreationActivation(
			INITIAL_WORKSPACE_CREATION_ACTIVATION_STATE,
			{ kind: 'mount' },
		).state;
		const disposed = transitionWorkspaceCreationActivation(
			transitionWorkspaceCreationActivation(mounted, { kind: 'request', target }).state,
			{ kind: 'dispose' },
		);
		expect(disposed).toEqual({ commands: [], state: { kind: 'disposed' } });
		expect(
			transitionWorkspaceCreationActivation(disposed.state, { kind: 'request', target }),
		).toEqual(disposed);
		expect(
			transitionWorkspaceCreationActivation(disposed.state, {
				kind: 'projection-observed',
				targets: [target],
			}),
		).toEqual(disposed);
		expect(transitionWorkspaceCreationActivation(disposed.state, { kind: 'mount' })).toEqual({
			commands: [],
			state: { kind: 'idle' },
		});
	});
});
