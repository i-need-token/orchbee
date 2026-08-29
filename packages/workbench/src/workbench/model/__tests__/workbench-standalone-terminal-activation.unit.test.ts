import { describe, expect, it } from 'vitest';

import {
	INITIAL_WORKBENCH_STANDALONE_TERMINAL_ACTIVATION_STATE,
	transitionWorkbenchStandaloneTerminalActivation,
	type WorkbenchStandaloneTerminalActivationState,
} from '../workbench-standalone-terminal-activation';

const workspaceLocation = {
	kind: 'workspace',
	projectId: 'project-1',
	workspaceId: 'workspace-1',
} as const;
const standaloneLocation = { kind: 'standalone' } as const;

describe('transitionWorkbenchStandaloneTerminalActivation', () => {
	it('focuses once when pointer activation settles before the standalone projection', () => {
		const requested = pointerRequest(mountedState(), 1);
		expect(requested.commands).toEqual([
			{ kind: 'invoke-standalone-activation', requestId: 1, source: 'pointer' },
		]);

		const accepted = transitionWorkbenchStandaloneTerminalActivation(requested.state, {
			kind: 'activation-settled',
			outcome: 'accepted',
			requestId: 1,
		});
		expect(accepted.commands).toEqual([]);

		const focused = observeStandalone(accepted.state, true);
		expect(focused).toEqual({
			commands: [{ kind: 'request-terminal-focus' }],
			state: { kind: 'idle' },
		});
		expect(observeStandalone(focused.state, true).commands).toEqual([]);
	});

	it('focuses once when the standalone projection arrives before pointer activation settles', () => {
		const requested = pointerRequest(mountedState(), 1);
		const projected = observeStandalone(requested.state, true);
		expect(projected.commands).toEqual([]);

		expect(
			transitionWorkbenchStandaloneTerminalActivation(projected.state, {
				kind: 'activation-settled',
				outcome: 'accepted',
				requestId: 1,
			}),
		).toEqual({
			commands: [{ kind: 'request-terminal-focus' }],
			state: { kind: 'idle' },
		});
	});

	it('waits for an exact available standalone terminal target', () => {
		const requested = pointerRequest(mountedState(), 1);
		const accepted = transitionWorkbenchStandaloneTerminalActivation(requested.state, {
			kind: 'activation-settled',
			outcome: 'accepted',
			requestId: 1,
		}).state;

		const unavailable = observeStandalone(accepted, false);
		expect(unavailable.commands).toEqual([]);
		expect(observeStandalone(unavailable.state, true).commands).toEqual([
			{ kind: 'request-terminal-focus' },
		]);
	});

	it('does not focus after failed activation or a different non-target projection', () => {
		const requested = pointerRequest(mountedState(), 1);
		const failed = transitionWorkbenchStandaloneTerminalActivation(requested.state, {
			kind: 'activation-settled',
			outcome: 'rejected',
			requestId: 1,
		});
		expect(failed).toEqual({ commands: [], state: { kind: 'idle' } });
		expect(observeStandalone(failed.state, true).commands).toEqual([]);

		const supersededLocation = transitionWorkbenchStandaloneTerminalActivation(
			pointerRequest(mountedState(), 2).state,
			{
				activeLocation: {
					kind: 'workspace',
					projectId: 'project-1',
					workspaceId: 'workspace-2',
				},
				kind: 'projection-observed',
				terminalTargetAvailable: true,
			},
		);
		expect(supersededLocation).toEqual({ commands: [], state: { kind: 'idle' } });
	});

	it('lets the latest pointer request win and ignores a stale settlement', () => {
		const first = pointerRequest(mountedState(), 1);
		const latest = pointerRequest(first.state, 2);
		const stale = transitionWorkbenchStandaloneTerminalActivation(latest.state, {
			kind: 'activation-settled',
			outcome: 'accepted',
			requestId: 1,
		});
		expect(stale).toEqual({ commands: [], state: latest.state });

		const projected = observeStandalone(stale.state, true);
		expect(
			transitionWorkbenchStandaloneTerminalActivation(projected.state, {
				kind: 'activation-settled',
				outcome: 'accepted',
				requestId: 2,
			}).commands,
		).toEqual([{ kind: 'request-terminal-focus' }]);
	});

	it('invokes keyboard activation without creating terminal focus intent', () => {
		const activation = transitionWorkbenchStandaloneTerminalActivation(mountedState(), {
			kind: 'activate',
			origin: 'other',
			originLocation: workspaceLocation,
			requestId: 1,
		});
		expect(activation).toEqual({
			commands: [{ kind: 'invoke-standalone-activation', requestId: 1, source: 'button' }],
			state: { kind: 'idle' },
		});
		expect(observeStandalone(activation.state, true).commands).toEqual([]);
		expect(
			transitionWorkbenchStandaloneTerminalActivation(activation.state, {
				kind: 'activation-settled',
				outcome: 'accepted',
				requestId: 1,
			}).commands,
		).toEqual([]);
	});

	it('drops pending focus intent on non-pointer activation and disposal', () => {
		const pointer = pointerRequest(mountedState(), 1);
		const keyboard = transitionWorkbenchStandaloneTerminalActivation(pointer.state, {
			kind: 'activate',
			origin: 'other',
			originLocation: workspaceLocation,
			requestId: 2,
		});
		expect(keyboard.state).toEqual({ kind: 'idle' });
		expect(
			transitionWorkbenchStandaloneTerminalActivation(keyboard.state, {
				kind: 'activation-settled',
				outcome: 'accepted',
				requestId: 1,
			}).commands,
		).toEqual([]);

		const disposed = transitionWorkbenchStandaloneTerminalActivation(pointer.state, {
			kind: 'dispose',
		});
		expect(disposed).toEqual({ commands: [], state: { kind: 'disposed' } });
		expect(observeStandalone(disposed.state, true)).toEqual(disposed);
	});
});

function mountedState(): WorkbenchStandaloneTerminalActivationState {
	return transitionWorkbenchStandaloneTerminalActivation(
		INITIAL_WORKBENCH_STANDALONE_TERMINAL_ACTIVATION_STATE,
		{ kind: 'mount' },
	).state;
}

function pointerRequest(state: WorkbenchStandaloneTerminalActivationState, requestId: number) {
	return transitionWorkbenchStandaloneTerminalActivation(state, {
		kind: 'activate',
		origin: 'pointer',
		originLocation: workspaceLocation,
		requestId,
	});
}

function observeStandalone(
	state: WorkbenchStandaloneTerminalActivationState,
	terminalTargetAvailable: boolean,
) {
	return transitionWorkbenchStandaloneTerminalActivation(state, {
		activeLocation: standaloneLocation,
		kind: 'projection-observed',
		terminalTargetAvailable,
	});
}
