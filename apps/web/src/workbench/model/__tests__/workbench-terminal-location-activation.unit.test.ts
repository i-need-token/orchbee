import { describe, expect, it } from 'vitest';

import {
	INITIAL_WORKBENCH_TERMINAL_LOCATION_ACTIVATION_STATE,
	transitionWorkbenchTerminalLocationActivation,
	type WorkbenchTerminalLocationActivationState,
} from '../workbench-terminal-location-activation';

const workspaceLocation = {
	kind: 'workspace',

	workspaceId: 'workspace-1',
} as const;
const otherWorkspaceLocation = {
	kind: 'workspace',

	workspaceId: 'workspace-2',
} as const;
const thirdWorkspaceLocation = { kind: 'workspace', workspaceId: 'workspace-3' } as const;

describe('transitionWorkbenchTerminalLocationActivation', () => {
	it('focuses once when pointer activation settles before the target Workspace projection', () => {
		const requested = pointerRequest(mountedState(), 1, thirdWorkspaceLocation);
		expect(requested.commands).toEqual([
			{
				kind: 'invoke-location-activation',
				requestId: 1,
				source: 'pointer',
				targetLocation: thirdWorkspaceLocation,
			},
		]);

		const accepted = transitionWorkbenchTerminalLocationActivation(requested.state, {
			kind: 'activation-settled',
			outcome: 'accepted',
			requestId: 1,
		});
		expect(accepted.commands).toEqual([]);

		const focused = observeThirdWorkspace(accepted.state, true);
		expect(focused).toEqual({
			commands: [{ kind: 'request-terminal-focus', targetLocation: thirdWorkspaceLocation }],
			state: { kind: 'idle' },
		});
		expect(observeThirdWorkspace(focused.state, true).commands).toEqual([]);
	});

	it('focuses a Workspace once when its projection arrives before pointer activation settles', () => {
		const requested = pointerRequest(mountedState(), 1, otherWorkspaceLocation);
		const projected = observeLocation(requested.state, otherWorkspaceLocation, true);
		expect(projected.commands).toEqual([]);

		expect(
			transitionWorkbenchTerminalLocationActivation(projected.state, {
				kind: 'activation-settled',
				outcome: 'accepted',
				requestId: 1,
			}),
		).toEqual({
			commands: [{ kind: 'request-terminal-focus', targetLocation: otherWorkspaceLocation }],
			state: { kind: 'idle' },
		});
	});

	it('waits for an exact available terminal target', () => {
		const requested = pointerRequest(mountedState(), 1, otherWorkspaceLocation);
		const accepted = transitionWorkbenchTerminalLocationActivation(requested.state, {
			kind: 'activation-settled',
			outcome: 'accepted',
			requestId: 1,
		}).state;

		const unavailable = observeLocation(accepted, otherWorkspaceLocation, false);
		expect(unavailable.commands).toEqual([]);
		expect(observeLocation(unavailable.state, otherWorkspaceLocation, true).commands).toEqual([
			{ kind: 'request-terminal-focus', targetLocation: otherWorkspaceLocation },
		]);
	});

	it('does not focus after failed activation or a different non-target projection', () => {
		const requested = pointerRequest(mountedState(), 1, thirdWorkspaceLocation);
		const failed = transitionWorkbenchTerminalLocationActivation(requested.state, {
			kind: 'activation-settled',
			outcome: 'rejected',
			requestId: 1,
		});
		expect(failed).toEqual({ commands: [], state: { kind: 'idle' } });
		expect(observeThirdWorkspace(failed.state, true).commands).toEqual([]);

		const supersededLocation = transitionWorkbenchTerminalLocationActivation(
			pointerRequest(mountedState(), 2, thirdWorkspaceLocation).state,
			{
				activeLocation: {
					kind: 'workspace',

					workspaceId: 'workspace-2',
				},
				kind: 'projection-observed',
				terminalTargetAvailable: true,
			},
		);
		expect(supersededLocation).toEqual({ commands: [], state: { kind: 'idle' } });
	});

	it('lets the latest pointer request win and ignores a stale settlement', () => {
		const first = pointerRequest(mountedState(), 1, thirdWorkspaceLocation);
		const latest = pointerRequest(first.state, 2, otherWorkspaceLocation);
		const stale = transitionWorkbenchTerminalLocationActivation(latest.state, {
			kind: 'activation-settled',
			outcome: 'accepted',
			requestId: 1,
		});
		expect(stale).toEqual({ commands: [], state: latest.state });

		const projected = observeLocation(stale.state, otherWorkspaceLocation, true);
		expect(
			transitionWorkbenchTerminalLocationActivation(projected.state, {
				kind: 'activation-settled',
				outcome: 'accepted',
				requestId: 2,
			}).commands,
		).toEqual([{ kind: 'request-terminal-focus', targetLocation: otherWorkspaceLocation }]);
	});

	it('invokes keyboard activation without creating terminal focus intent', () => {
		const activation = transitionWorkbenchTerminalLocationActivation(mountedState(), {
			kind: 'activate',
			origin: 'other',
			originLocation: workspaceLocation,
			requestId: 1,
			targetLocation: thirdWorkspaceLocation,
		});
		expect(activation).toEqual({
			commands: [
				{
					kind: 'invoke-location-activation',
					requestId: 1,
					source: 'button',
					targetLocation: thirdWorkspaceLocation,
				},
			],
			state: { kind: 'idle' },
		});
		expect(observeThirdWorkspace(activation.state, true).commands).toEqual([]);
		expect(
			transitionWorkbenchTerminalLocationActivation(activation.state, {
				kind: 'activation-settled',
				outcome: 'accepted',
				requestId: 1,
			}).commands,
		).toEqual([]);
	});

	it('drops pending focus intent on non-pointer activation and disposal', () => {
		const pointer = pointerRequest(mountedState(), 1, thirdWorkspaceLocation);
		const keyboard = transitionWorkbenchTerminalLocationActivation(pointer.state, {
			kind: 'activate',
			origin: 'other',
			originLocation: workspaceLocation,
			requestId: 2,
			targetLocation: otherWorkspaceLocation,
		});
		expect(keyboard.state).toEqual({ kind: 'idle' });
		expect(
			transitionWorkbenchTerminalLocationActivation(keyboard.state, {
				kind: 'activation-settled',
				outcome: 'accepted',
				requestId: 1,
			}).commands,
		).toEqual([]);

		const disposed = transitionWorkbenchTerminalLocationActivation(pointer.state, {
			kind: 'dispose',
		});
		expect(disposed).toEqual({ commands: [], state: { kind: 'disposed' } });
		expect(observeThirdWorkspace(disposed.state, true)).toEqual(disposed);
	});
});

function mountedState(): WorkbenchTerminalLocationActivationState {
	return transitionWorkbenchTerminalLocationActivation(
		INITIAL_WORKBENCH_TERMINAL_LOCATION_ACTIVATION_STATE,
		{ kind: 'mount' },
	).state;
}

function pointerRequest(
	state: WorkbenchTerminalLocationActivationState,
	requestId: number,
	targetLocation: typeof thirdWorkspaceLocation | typeof otherWorkspaceLocation,
) {
	return transitionWorkbenchTerminalLocationActivation(state, {
		kind: 'activate',
		origin: 'pointer',
		originLocation: workspaceLocation,
		requestId,
		targetLocation,
	});
}

function observeThirdWorkspace(
	state: WorkbenchTerminalLocationActivationState,
	terminalTargetAvailable: boolean,
) {
	return observeLocation(state, thirdWorkspaceLocation, terminalTargetAvailable);
}

function observeLocation(
	state: WorkbenchTerminalLocationActivationState,
	activeLocation: typeof thirdWorkspaceLocation | typeof otherWorkspaceLocation,
	terminalTargetAvailable: boolean,
) {
	return transitionWorkbenchTerminalLocationActivation(state, {
		activeLocation,
		kind: 'projection-observed',
		terminalTargetAvailable,
	});
}
