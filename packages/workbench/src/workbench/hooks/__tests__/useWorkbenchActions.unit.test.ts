// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import * as React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { WORKBENCH_ACTION_IDS } from '../../actions/workbench-action-ids';
import type { WorkbenchTerminalCreateCallback } from '../../types/workbench-host-actions';
import { useWorkbenchActions } from '../useWorkbenchActions';

interface WorkbenchActionsHarnessProps {
	readonly captureScope?: ((scope: ReturnType<ActionRuntime['createScope']>) => void) | undefined;
	readonly onNewTerminal?: WorkbenchTerminalCreateCallback | undefined;
	readonly runtime: ActionRuntime;
}

function WorkbenchActionsHarness({
	captureScope,
	onNewTerminal,
	runtime,
}: WorkbenchActionsHarnessProps): React.ReactNode {
	const [isSidebarVisible, setIsSidebarVisible] = React.useState(true);
	const actions = useWorkbenchActions({
		isSidebarVisible,
		onNewTerminal,
		onSettings: undefined,
		openSearch: () => undefined,
		runtime,
		setTabStripOrientation: () => undefined,
		tabStripOrientation: 'horizontal',
		toggleSidebar: () => setIsSidebarVisible((visible) => !visible),
	});
	captureScope?.(actions.scope);

	return React.createElement(
		React.Fragment,
		null,
		React.createElement(
			'button',
			{
				onClick: () =>
					actions.invoke(WORKBENCH_ACTION_IDS.createTerminal, { groupId: null }, 'button'),
				type: 'button',
			},
			'create',
		),
		React.createElement(
			'button',
			{
				onClick: () => actions.invoke(WORKBENCH_ACTION_IDS.toggleSidebar, undefined, 'button'),
				type: 'button',
			},
			'toggle',
		),
		React.createElement('output', null, isSidebarVisible ? 'visible' : 'hidden'),
	);
}

afterEach(() => cleanup());

describe('useWorkbenchActions', () => {
	it('registers enumerable actions and routes product controls through their handlers', async () => {
		const runtime = new ActionRuntime();
		const onNewTerminal = vi.fn<WorkbenchTerminalCreateCallback>(() => ({
			kind: 'completed',
		}));
		let actionScope: ReturnType<ActionRuntime['createScope']> | undefined;
		render(
			React.createElement(WorkbenchActionsHarness, {
				captureScope: (scope) => {
					actionScope = scope;
				},
				onNewTerminal,
				runtime,
			}),
		);
		expect(actionScope).toBeDefined();
		if (actionScope === undefined) {
			throw new Error('Expected the Workbench action scope to be captured');
		}
		expect(
			new Set(
				runtime
					.list(
						{
							scope: actionScope,
							source: 'command-surface',
						},
						{
							[WORKBENCH_ACTION_IDS.openSearch]: {
								filter: 'all',
							},
							[WORKBENCH_ACTION_IDS.setTabStripOrientation]: {
								orientation: 'horizontal',
							},
						},
					)
					.map((action) => action.descriptor.id),
			),
		).toEqual(new Set(Object.values(WORKBENCH_ACTION_IDS)));

		fireEvent.click(screen.getByRole('button', { name: 'create' }));
		fireEvent.click(screen.getByRole('button', { name: 'toggle' }));

		expect(onNewTerminal).toHaveBeenCalledOnce();
		expect(onNewTerminal).toHaveBeenCalledWith(expect.any(Object), { groupId: null });
		expect(screen.getByText('hidden')).toBeDefined();
		await act(async () => undefined);
	});

	it('keeps missing host capabilities disabled for shortcut resolution', () => {
		const runtime = new ActionRuntime();
		let capturedScope: ReturnType<typeof runtime.createScope> | undefined;
		const CapturingHarness = () => {
			const actions = useWorkbenchActions({
				isSidebarVisible: true,
				onNewTerminal: undefined,
				onSettings: undefined,
				openSearch: () => undefined,
				runtime,
				setTabStripOrientation: () => undefined,
				tabStripOrientation: 'horizontal',
				toggleSidebar: () => undefined,
			});
			capturedScope = actions.scope;
			return null;
		};
		render(React.createElement(CapturingHarness));
		expect(capturedScope).toBeDefined();
		if (capturedScope === undefined) {
			throw new Error('Expected the Workbench action scope to be captured');
		}

		expect(
			runtime.resolveAction(WORKBENCH_ACTION_IDS.createTerminal, undefined, {
				scope: capturedScope,
				source: 'keyboard',
			}),
		).toMatchObject({
			availability: { kind: 'disabled' },
		});
	});

	it('passes cancellation and terminal-result ownership to an asynchronous host operation', async () => {
		const runtime = new ActionRuntime();
		let actionScope: ReturnType<ActionRuntime['createScope']> | undefined;
		let context: Parameters<WorkbenchTerminalCreateCallback>[0] | undefined;
		let settle:
			| ((result: Awaited<ReturnType<WorkbenchTerminalCreateCallback>>) => void)
			| undefined;
		const onNewTerminal: WorkbenchTerminalCreateCallback = (candidateContext) => {
			context = candidateContext;
			return new Promise((resolve) => {
				settle = resolve;
			});
		};
		render(
			React.createElement(WorkbenchActionsHarness, {
				captureScope: (scope) => {
					actionScope = scope;
				},
				onNewTerminal,
				runtime,
			}),
		);
		if (actionScope === undefined) {
			throw new Error('Expected the Workbench action scope to be captured');
		}

		const execution = runtime.execute(
			WORKBENCH_ACTION_IDS.createTerminal,
			{ groupId: null },
			{ scope: actionScope, source: 'button' },
		);
		expect(execution.getSnapshot().state).toBe('running');
		execution.cancel();

		expect(context?.signal.aborted).toBe(true);
		expect(execution.getSnapshot().state).toBe('cancelling');
		settle?.({ kind: 'cancelled' });
		await expect(execution.completion).resolves.toEqual({ kind: 'cancelled' });
	});

	it('preserves a rejected host operation as the terminal Action failure', async () => {
		const runtime = new ActionRuntime();
		let actionScope: ReturnType<ActionRuntime['createScope']> | undefined;
		render(
			React.createElement(WorkbenchActionsHarness, {
				captureScope: (scope) => {
					actionScope = scope;
				},
				onNewTerminal: () => Promise.reject(new Error('terminal creation failed')),
				runtime,
			}),
		);
		if (actionScope === undefined) {
			throw new Error('Expected the Workbench action scope to be captured');
		}

		await expect(
			runtime.execute(WORKBENCH_ACTION_IDS.createTerminal, undefined, {
				scope: actionScope,
				source: 'button',
			}).completion,
		).resolves.toEqual({
			failure: { code: 'unexpected-error' },
			kind: 'failed',
		});
	});
});
