import { type ReactNode } from 'react';
import { type TabData, type TabModelSnapshot } from '../../../../../../model/tab-model';
import { TabStripOrientation } from '../../../../../../types/tab-strip-types';
import { TabButton } from '../../../tab-button/TabButton';
import { tabButtonContext } from '../../../tab-button/utils/tabButtonContext';

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
	const tabContext = tabButtonContext(tab, snapshot, null);
	return (
		<TabButton
			tab={tab}
			active={tabContext.active}
			selected={tabContext.selected}
			group={tabContext.group}
			tabPositionIndex={tabContext.tabPositionIndex}
			tabCount={tabContext.tabCount}
			hoveredByStrip={tabContext.hoveredByStrip}
			previousTab={tabContext.previousTab}
			previousActive={tabContext.previousActive}
			previousSelected={tabContext.previousSelected}
			previousTabHovered={tabContext.previousTabHovered}
			nextTab={tabContext.nextTab}
			nextActive={tabContext.nextActive}
			nextSelected={tabContext.nextSelected}
			nextTabHovered={tabContext.nextTabHovered}
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
			tabIndex={-1}
			touchUi={touchUi}
			closeButtonAllowed={closeButtonAllowed}
			dragging
		/>
	);
}
