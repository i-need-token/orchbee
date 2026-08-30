// @vitest-environment jsdom

import type { ActionExecution, ActionExecutionSnapshot, ActionResult } from '@orchbee/actions';
import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchActiveLocation } from '../../../project-workspace';
import { useWorkbenchTerminalLocationActivation } from '../useWorkbenchTerminalLocationActivation';

afterEach(cleanup);

describe('useWorkbenchTerminalLocationActivation', () => {
	it('waits for late Workspace publication before requesting focus', async () => {
		const pending = pendingExecution('activation-1');
		const invokeLocationActivation = vi.fn<
			(targetLocation: WorkbenchActiveLocation, source: 'button' | 'pointer') => ActionExecution
		>(() => pending.execution);
		const requestTerminalFocus = vi.fn<(targetLocation: WorkbenchActiveLocation) => boolean>(
			() => true,
		);
		let properties = standaloneProjection();
		const rendered = renderHook(
			({ activeLocation, terminalTargetAvailable }) =>
				useWorkbenchTerminalLocationActivation({
					activeLocation,
					invokeLocationActivation,
					requestTerminalFocus,
					terminalTargetAvailable,
				}),
			{ initialProps: properties },
		);

		act(() => rendered.result.current(workspaceLocation, 'pointer'));
		expect(invokeLocationActivation).toHaveBeenCalledWith(workspaceLocation, 'pointer');
		expect(requestTerminalFocus).not.toHaveBeenCalled();

		await pending.settle({ kind: 'completed' });
		expect(requestTerminalFocus).not.toHaveBeenCalled();

		properties = { activeLocation: workspaceLocation, terminalTargetAvailable: true };
		rendered.rerender(properties);
		expect(requestTerminalFocus).toHaveBeenCalledWith(workspaceLocation);
		rendered.rerender(properties);
		expect(requestTerminalFocus).toHaveBeenCalledOnce();
	});

	it('waits for the Action result when standalone publication arrives first', async () => {
		const pending = pendingExecution('activation-1');
		const requestTerminalFocus = vi.fn<(targetLocation: WorkbenchActiveLocation) => boolean>(
			() => true,
		);
		let properties = workspaceProjection();
		const rendered = renderHook(
			({ activeLocation, terminalTargetAvailable }) =>
				useWorkbenchTerminalLocationActivation({
					activeLocation,
					invokeLocationActivation: () => pending.execution,
					requestTerminalFocus,
					terminalTargetAvailable,
				}),
			{ initialProps: properties },
		);

		act(() => rendered.result.current(standaloneLocation, 'pointer'));
		properties = { activeLocation: { kind: 'standalone' }, terminalTargetAvailable: true };
		rendered.rerender(properties);
		expect(requestTerminalFocus).not.toHaveBeenCalled();

		await pending.settle({ kind: 'unchanged' });
		expect(requestTerminalFocus).toHaveBeenCalledOnce();
	});

	it('keeps keyboard activation and rejected pointer activation focus-neutral', async () => {
		const keyboard = pendingExecution('keyboard-activation');
		const pointer = pendingExecution('pointer-activation');
		const executions = [keyboard.execution, pointer.execution];
		const invokeLocationActivation = vi.fn<
			(targetLocation: WorkbenchActiveLocation, source: 'button' | 'pointer') => ActionExecution
		>(() => {
			const execution = executions.shift();
			if (execution === undefined) throw new Error('Expected a pending activation.');
			return execution;
		});
		const requestTerminalFocus = vi.fn<(targetLocation: WorkbenchActiveLocation) => boolean>(
			() => true,
		);
		const rendered = renderHook(() =>
			useWorkbenchTerminalLocationActivation({
				activeLocation: { kind: 'standalone' },
				invokeLocationActivation,
				requestTerminalFocus,
				terminalTargetAvailable: true,
			}),
		);

		act(() => rendered.result.current(workspaceLocation, 'other'));
		await keyboard.settle({ kind: 'completed' });
		expect(invokeLocationActivation).toHaveBeenNthCalledWith(1, workspaceLocation, 'button');
		expect(requestTerminalFocus).not.toHaveBeenCalled();

		act(() => rendered.result.current(standaloneLocation, 'pointer'));
		await pointer.settle({ kind: 'cancelled' });
		expect(requestTerminalFocus).not.toHaveBeenCalled();
	});

	it('ignores a superseded completion and every completion after unmount', async () => {
		const first = pendingExecution('activation-1');
		const latest = pendingExecution('activation-2');
		const executions = [first.execution, latest.execution];
		const requestTerminalFocus = vi.fn<(targetLocation: WorkbenchActiveLocation) => boolean>(
			() => true,
		);
		const rendered = renderHook(() =>
			useWorkbenchTerminalLocationActivation({
				activeLocation: { kind: 'standalone' },
				invokeLocationActivation: () => {
					const execution = executions.shift();
					if (execution === undefined) throw new Error('Expected a pending activation.');
					return execution;
				},
				requestTerminalFocus,
				terminalTargetAvailable: true,
			}),
		);

		act(() => {
			rendered.result.current(workspaceLocation, 'pointer');
			rendered.result.current(standaloneLocation, 'pointer');
		});
		await first.settle({ kind: 'completed' });
		expect(requestTerminalFocus).not.toHaveBeenCalled();

		rendered.unmount();
		await latest.settle({ kind: 'completed' });
		expect(requestTerminalFocus).not.toHaveBeenCalled();
	});
});

const standaloneLocation = { kind: 'standalone' } as const;
const workspaceLocation = {
	kind: 'workspace',
	projectId: 'project-1',
	workspaceId: 'workspace-2',
} as const;

function workspaceProjection(): {
	readonly activeLocation: WorkbenchActiveLocation;
	readonly terminalTargetAvailable: boolean;
} {
	return {
		activeLocation: {
			kind: 'workspace',
			projectId: 'project-1',
			workspaceId: 'workspace-1',
		},
		terminalTargetAvailable: true,
	};
}

function standaloneProjection(): {
	readonly activeLocation: WorkbenchActiveLocation;
	readonly terminalTargetAvailable: boolean;
} {
	return {
		activeLocation: standaloneLocation,
		terminalTargetAvailable: true,
	};
}

function pendingExecution(executionId: string): {
	readonly execution: ActionExecution;
	settle(result: ActionResult): Promise<void>;
} {
	let resolveCompletion: ((result: ActionResult) => void) | undefined;
	let snapshot: ActionExecutionSnapshot = {
		progress: null,
		result: null,
		state: 'running',
	};
	const completion = new Promise<ActionResult>((resolve) => {
		resolveCompletion = resolve;
	});
	return {
		execution: {
			cancel: () => undefined,
			completion,
			executionId,
			getSnapshot: () => snapshot,
			subscribe: () => ({ dispose: () => undefined }),
		},
		settle: async (result) => {
			snapshot = { progress: null, result, state: result.kind };
			await act(async () => {
				resolveCompletion?.(result);
				await completion;
			});
		},
	};
}
