import { DragSessionData, DragSlotViewType, TabModel } from '@orchbee/tab-strip';
import { describe, expect, it, vi } from 'vitest';

import type {
	WorkbenchTabWindowControl,
	WorkbenchTabWindowDestination,
} from '../../types/workbench-tab-window-control';
import { WorkbenchTabWindowDragRuntime } from '../WorkbenchTabWindowDragRuntime';

describe('WorkbenchTabWindowDragRuntime', () => {
	it('restores the pre-drag shared layout before committing an external move', () => {
		const model = createModel();
		const control = createControl();
		const runtime = new WorkbenchTabWindowDragRuntime({
			control,
			model,
			sessionData: dragSession('tab-1'),
			terminalContextId: 'context-1',
		});

		runtime.start();
		model.moveTabLast('tab-1');
		control.setTarget({ kind: 'existing', windowId: 'desktop-window-2' });
		runtime.update({ x: 1_200, y: 100 });
		runtime.end('complete');

		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['tab-1', 'tab-2', 'tab-3']);
		expect(control.startDrag).toHaveBeenCalledWith('context-1', ['tab-1']);
		expect(control.updateDrag).toHaveBeenCalledWith('drag-1', { x: 1_200, y: 100 });
		expect(control.finishDrag).toHaveBeenCalledWith('drag-1', {
			point: { x: 1_200, y: 100 },
			reason: 'complete',
		});
		expect(runtime.getSnapshot()).toMatchObject({
			endReason: 'complete',
			state: 'stopped',
			targetActive: false,
		});
	});

	it('leaves a local reorder intact when no external target accepts the drop', () => {
		const model = createModel();
		const control = createControl();
		const runtime = new WorkbenchTabWindowDragRuntime({
			control,
			model,
			sessionData: dragSession('tab-1'),
			terminalContextId: 'context-1',
		});

		runtime.start();
		model.moveTabLast('tab-1');
		runtime.update({ x: 300, y: 100 });
		runtime.end('complete');

		expect(model.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['tab-2', 'tab-3', 'tab-1']);
		expect(control.finishDrag).toHaveBeenCalledWith('drag-1', { reason: 'cancel' });
	});
});

function createControl() {
	let listener: (() => void) | undefined;
	let target: WorkbenchTabWindowDestination | null = null;
	return {
		finishDrag: vi
			.fn<WorkbenchTabWindowControl['finishDrag']>()
			.mockResolvedValue({ kind: 'unchanged' }),
		getDragTarget: vi
			.fn<WorkbenchTabWindowControl['getDragTarget']>()
			.mockImplementation(() => target),
		getSnapshot: vi.fn<WorkbenchTabWindowControl['getSnapshot']>().mockReturnValue({
			currentWindowId: 'desktop-window-1',
			revision: '0',
			terminalContexts: [],
			windows: [
				{ ordinal: 1, windowId: 'desktop-window-1' },
				{ ordinal: 2, windowId: 'desktop-window-2' },
			],
		}),
		moveGroup: vi
			.fn<WorkbenchTabWindowControl['moveGroup']>()
			.mockResolvedValue({ kind: 'unchanged' }),
		moveTabs: vi
			.fn<WorkbenchTabWindowControl['moveTabs']>()
			.mockResolvedValue({ kind: 'unchanged' }),
		setTarget(nextTarget: WorkbenchTabWindowDestination | null): void {
			target = nextTarget;
			listener?.();
		},
		startDrag: vi.fn<WorkbenchTabWindowControl['startDrag']>().mockReturnValue('drag-1'),
		subscribe: vi.fn<WorkbenchTabWindowControl['subscribe']>((nextListener) => {
			listener = nextListener;
			return () => {
				listener = undefined;
			};
		}),
		updateDrag: vi.fn<WorkbenchTabWindowControl['updateDrag']>(),
	};
}

function createModel(): TabModel {
	return new TabModel({
		activeTabId: 'tab-1',
		tabs: [
			{ group: null, id: 'tab-1', pinned: false, title: 'One' },
			{ group: null, id: 'tab-2', pinned: false, title: 'Two' },
			{ group: null, id: 'tab-3', pinned: false, title: 'Three' },
		],
	});
}

function dragSession(tabId: string): DragSessionData {
	return new DragSessionData({
		mouseOffsetToSizeRatios: { x: 0.5, y: 0.5 },
		sourceViewIndex: 0,
		tabDragData: [
			{
				attachedViewId: tabId,
				contentsId: tabId,
				pinned: false,
				sourceModelIndex: 0,
				tabGroupData: null,
				viewType: DragSlotViewType.Tab,
			},
		],
	});
}
