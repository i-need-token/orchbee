import { describe, expect, it } from 'vitest';
import { DragSessionData } from '../drag-session-data';

describe('DragSessionData', () => {
	it('records one global drag-start time without restarting it', () => {
		const session = new DragSessionData({
			tabDragData: [
				{
					sourceModelIndex: 0,
					viewType: 'tab',
					contentsId: 'a',
					attachedViewId: 'a',
					pinned: false,
					tabGroupData: null,
				},
			],
			sourceViewIndex: 0,
			mouseOffsetToSizeRatios: { x: 0.5, y: 0.5 },
		});
		expect(session.hasDragStarted).toBe(false);
		session.markDragStarted(100);
		session.markDragStarted(200);
		expect(session.hasDragStarted).toBe(true);
		expect(session.dragStartTime).toBe(100);
	});

	it('keeps the drag-start compaction decision with the global session', () => {
		const session = new DragSessionData({
			tabDragData: [
				{
					sourceModelIndex: 0,
					viewType: 'tab',
					contentsId: 'a',
					attachedViewId: 'a',
					pinned: false,
					tabGroupData: null,
				},
			],
			sourceViewIndex: 0,
			mouseOffsetToSizeRatios: { x: 0.5, y: 0.5 },
		});
		session.markDragStarted(100, false);
		expect(session.dragStartAnimationRequired).toBe(false);
	});
});
