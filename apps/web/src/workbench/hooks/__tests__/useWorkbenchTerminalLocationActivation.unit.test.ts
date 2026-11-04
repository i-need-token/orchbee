// @vitest-environment jsdom

import type { ActionExecution } from '@orchbee/actions';
import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, startTransition, Suspense } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchActiveLocation } from '../../../project-workspace';
import { useWorkbenchTerminalLocationActivation } from '../useWorkbenchTerminalLocationActivation';
import { pendingActionExecution } from './pendingActionExecution';

afterEach(cleanup);

describe('useWorkbenchTerminalLocationActivation', () => {
	it('waits for a committed terminal projection before focusing after activation', async () => {
		const execution = pendingActionExecution('suspended-activation');
		const requestTerminalFocus = vi.fn<(target: WorkbenchActiveLocation) => boolean>(() => true);
		const pending = new Promise<void>(() => undefined);
		const rendered = renderHook(
			({ projection, suspend }) => {
				const activate = useWorkbenchTerminalLocationActivation({
					...projection,
					invokeLocationActivation: () => execution.execution,
					requestTerminalFocus,
				});
				if (suspend) throw pending;
				return activate;
			},
			{
				initialProps: { projection: thirdWorkspaceProjection(), suspend: false },
				wrapper: ({ children }) => createElement(Suspense, { fallback: null }, children),
			},
		);
		const projection = { activeLocation: workspaceLocation, terminalTargetAvailable: true };
		act(() => {
			startTransition(() => rendered.rerender({ projection, suspend: true }));
		});
		act(() => rendered.result.current(workspaceLocation, 'pointer'));
		await execution.settle({ kind: 'completed' });
		expect(requestTerminalFocus).not.toHaveBeenCalled();
		rendered.rerender({ projection, suspend: false });
		expect(requestTerminalFocus).toHaveBeenCalledExactlyOnceWith(workspaceLocation);
	});

	it('waits for late Workspace publication before requesting focus', async () => {
		const pending = pendingActionExecution('activation-1');
		const invokeLocationActivation = vi.fn<
			(targetLocation: WorkbenchActiveLocation, source: 'button' | 'pointer') => ActionExecution
		>(() => pending.execution);
		const requestTerminalFocus = vi.fn<(targetLocation: WorkbenchActiveLocation) => boolean>(
			() => true,
		);
		let properties = thirdWorkspaceProjection();
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

	it('waits for the Action result when the target Workspace publication arrives first', async () => {
		const pending = pendingActionExecution('activation-1');
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

		act(() => rendered.result.current(thirdWorkspaceLocation, 'pointer'));
		properties = { activeLocation: thirdWorkspaceLocation, terminalTargetAvailable: true };
		rendered.rerender(properties);
		expect(requestTerminalFocus).not.toHaveBeenCalled();

		await pending.settle({ kind: 'unchanged' });
		expect(requestTerminalFocus).toHaveBeenCalledOnce();
	});

	it('keeps keyboard activation and rejected pointer activation focus-neutral', async () => {
		const keyboard = pendingActionExecution('keyboard-activation');
		const pointer = pendingActionExecution('pointer-activation');
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
				activeLocation: thirdWorkspaceLocation,
				invokeLocationActivation,
				requestTerminalFocus,
				terminalTargetAvailable: true,
			}),
		);

		act(() => rendered.result.current(workspaceLocation, 'other'));
		await keyboard.settle({ kind: 'completed' });
		expect(invokeLocationActivation).toHaveBeenNthCalledWith(1, workspaceLocation, 'button');
		expect(requestTerminalFocus).not.toHaveBeenCalled();

		act(() => rendered.result.current(thirdWorkspaceLocation, 'pointer'));
		await pointer.settle({ kind: 'cancelled' });
		expect(requestTerminalFocus).not.toHaveBeenCalled();
	});

	it('ignores a superseded completion and every completion after unmount', async () => {
		const first = pendingActionExecution('activation-1');
		const latest = pendingActionExecution('activation-2');
		const executions = [first.execution, latest.execution];
		const requestTerminalFocus = vi.fn<(targetLocation: WorkbenchActiveLocation) => boolean>(
			() => true,
		);
		const rendered = renderHook(() =>
			useWorkbenchTerminalLocationActivation({
				activeLocation: thirdWorkspaceLocation,
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
			rendered.result.current(thirdWorkspaceLocation, 'pointer');
		});
		await first.settle({ kind: 'completed' });
		expect(requestTerminalFocus).not.toHaveBeenCalled();

		rendered.unmount();
		await latest.settle({ kind: 'completed' });
		expect(requestTerminalFocus).not.toHaveBeenCalled();
	});
});

const thirdWorkspaceLocation = { kind: 'workspace', workspaceId: 'workspace-3' } as const;
const workspaceLocation = {
	kind: 'workspace',

	workspaceId: 'workspace-2',
} as const;

function workspaceProjection(): {
	readonly activeLocation: WorkbenchActiveLocation;
	readonly terminalTargetAvailable: boolean;
} {
	return {
		activeLocation: {
			kind: 'workspace',

			workspaceId: 'workspace-1',
		},
		terminalTargetAvailable: true,
	};
}

function thirdWorkspaceProjection(): {
	readonly activeLocation: WorkbenchActiveLocation;
	readonly terminalTargetAvailable: boolean;
} {
	return {
		activeLocation: thirdWorkspaceLocation,
		terminalTargetAvailable: true,
	};
}
