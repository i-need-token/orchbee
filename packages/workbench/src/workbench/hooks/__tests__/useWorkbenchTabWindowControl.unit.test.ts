import { ActionRuntime, defineAction, noActionInput } from '@orchbee/actions';
import { TabModel } from '@orchbee/tab-strip';
import { createElement, type ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import type { WorkbenchTabWindowControl } from '../../types/workbench-tab-window-control';
import { useWorkbenchTabWindowControl } from '../useWorkbenchTabWindowControl';

interface WindowControlProbeProps {
	readonly capture: (value: ReturnType<typeof useWorkbenchTabWindowControl>) => void;
	readonly control: WorkbenchTabWindowControl | undefined;
}

function WindowControlProbe({ capture, control }: WindowControlProbeProps): ReactNode {
	capture(
		useWorkbenchTabWindowControl({
			control,
			formatWindowLabel: (ordinal) => `Window ${ordinal}`,
			model: new TabModel(),
			terminalContextId: 'context-1',
		}),
	);
	return null;
}

describe('useWorkbenchTabWindowControl', () => {
	it('projects other windows and commits successful tab moves through the host control', async () => {
		const control = createControl();
		let integration: ReturnType<typeof useWorkbenchTabWindowControl> | undefined;
		renderToStaticMarkup(
			createElement(WindowControlProbe, {
				capture: (value) => {
					integration = value;
				},
				control,
			}),
		);
		if (integration === undefined) throw new Error('Expected the Hook integration result.');

		expect(integration.windowTargets).toEqual([
			{ label: 'Window 2', windowId: 'desktop-window-2' },
		]);
		expect(integration.createCrossWindowDragRuntime).toBeTypeOf('function');
		const moveTabsToWindow = integration.moveTabsToWindow;
		if (moveTabsToWindow === undefined) throw new Error('Expected the tab move callback.');
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('window-control-test');
		runtime.register(
			scope,
			defineAction({
				category: { id: 'test', title: { defaultMessage: 'Test' } },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'move-tabs',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: (context) =>
						moveTabsToWindow(context, {
							destinationWindowId: 'desktop-window-2',
							tabIds: ['tab-1'],
						}),
				},
				id: 'test.move-tabs',
				input: noActionInput,
				title: { defaultMessage: 'Move tabs' },
			}),
		);

		await expect(
			runtime.execute('test.move-tabs', undefined, { scope, source: 'button' }).completion,
		).resolves.toEqual({ kind: 'completed' });
		expect(control.moveTabs).toHaveBeenCalledWith('context-1', ['tab-1'], {
			kind: 'existing',
			windowId: 'desktop-window-2',
		});
	});

	it('publishes no window actions when the native host capability is absent', () => {
		let integration: ReturnType<typeof useWorkbenchTabWindowControl> | undefined;
		renderToStaticMarkup(
			createElement(WindowControlProbe, {
				capture: (value) => {
					integration = value;
				},
				control: undefined,
			}),
		);

		expect(integration).toMatchObject({
			createCrossWindowDragRuntime: undefined,
			moveGroupToWindow: undefined,
			moveTabsToWindow: undefined,
			windowTargets: [],
		});
	});
});

function createControl(): WorkbenchTabWindowControl {
	return {
		finishDrag: vi.fn<WorkbenchTabWindowControl['finishDrag']>().mockResolvedValue({
			kind: 'unchanged',
		}),
		getDragTarget: vi.fn<WorkbenchTabWindowControl['getDragTarget']>().mockReturnValue(null),
		getSnapshot: vi.fn<WorkbenchTabWindowControl['getSnapshot']>().mockReturnValue({
			currentWindowId: 'desktop-window-1',
			revision: '1',
			terminalContexts: [{ tabIds: ['tab-1'], terminalContextId: 'context-1' }],
			windows: [
				{ ordinal: 1, windowId: 'desktop-window-1' },
				{ ordinal: 2, windowId: 'desktop-window-2' },
			],
		}),
		moveGroup: vi.fn<WorkbenchTabWindowControl['moveGroup']>().mockResolvedValue({
			kind: 'unchanged',
		}),
		moveTabs: vi.fn<WorkbenchTabWindowControl['moveTabs']>().mockResolvedValue({
			destinationWindowId: 'desktop-window-2',
			kind: 'completed',
		}),
		startDrag: vi.fn<WorkbenchTabWindowControl['startDrag']>().mockReturnValue('drag-1'),
		subscribe: vi.fn<WorkbenchTabWindowControl['subscribe']>(() => () => undefined),
		updateDrag: vi.fn<WorkbenchTabWindowControl['updateDrag']>(),
	};
}
