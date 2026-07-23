import { type ReactNode } from 'react';
import { type TabData, type TabModelSnapshot } from '../../../../../../model/tab-model';
import { TabStripOrientation } from '../../../../../../types/tab-strip-types';
import { TabButton } from '../../../tab-button/TabButton';

export function VerticalDragTabVisual({
	tab,
	snapshot,
	width,
	collapsed,
	touchUi,
	closeButtonAllowed,
}: {
	tab: Readonly<TabData>;
	snapshot: TabModelSnapshot;
	width: number;
	collapsed: boolean;
	touchUi: boolean;
	closeButtonAllowed: boolean;
}): ReactNode {
	return (
		<TabButton
			tab={tab}
			snapshot={snapshot}
			width={width}
			orientation={TabStripOrientation.Vertical}
			collapsed={collapsed}
			onPointerDown={() => undefined}
			onClose={() => undefined}
			onContextMenu={() => undefined}
			onMouseEnter={() => undefined}
			onMouseLeave={() => undefined}
			onKeyDown={() => undefined}
			onKeyUp={() => undefined}
			touchUi={touchUi}
			closeButtonAllowed={closeButtonAllowed}
			hoveredTabId={null}
			dragging
		/>
	);
}
