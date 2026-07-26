import {
	buildDraggedTabsLayout,
	getDraggingViewsBoundsAtPoint,
} from '../../../../../dragging/dragged-tabs-container-layout';
import {
	getSourceVisualId,
	getVerticalDragVisualViews,
	type DragState,
} from '../../../../../utils/tab-strip-internals';
import type { Rect } from '../../../../../utils/rectRight';

export function calculateVerticalDragCompositeBounds({
	collapsed,
	drag,
	sourcePinned,
	viewportBounds,
}: {
	collapsed: boolean;
	drag: Pick<
		DragState,
		| 'groupHeaderId'
		| 'groupWasCollapsedBeforeDrag'
		| 'pointerOffsetRatios'
		| 'sourceId'
		| 'sourceSlotSize'
		| 'sourceViewOffsetFromSlot'
		| 'visualViews'
		| 'x'
		| 'y'
	>;
	sourcePinned: boolean;
	viewportBounds: Readonly<Rect>;
}): Rect {
	const sourceVisualId = getSourceVisualId(drag);
	const visualViews = getVerticalDragVisualViews(drag);
	const layout = buildDraggedTabsLayout({
		layout: collapsed || sourcePinned ? 'squash' : 'vertical',
		pointerOffsetRatios: drag.pointerOffsetRatios,
		sourceSlotSize: drag.sourceSlotSize,
		sourceViewOffsetFromSlot: drag.sourceViewOffsetFromSlot,
		views: visualViews.map((view) => ({
			id: view.id,
			bounds: { x: view.x, y: view.y, width: view.width, height: view.height },
			source: view.id === sourceVisualId,
		})),
	});
	const bounds = getDraggingViewsBoundsAtPoint(layout.boundsRelativeToPointer, {
		x: drag.x,
		y: drag.y,
	});
	return {
		...bounds,
		x: bounds.x - viewportBounds.x,
		y: bounds.y - viewportBounds.y,
	};
}
