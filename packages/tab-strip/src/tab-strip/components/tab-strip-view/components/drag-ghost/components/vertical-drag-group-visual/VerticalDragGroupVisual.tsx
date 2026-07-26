import styles from './VerticalDragGroupVisual.module.css';
import { type CSSProperties, type ReactNode } from 'react';
import {
	type TabData,
	type TabGroupData,
	type TabModelSnapshot,
} from '../../../../../../model/tab-model';
import { TabStripOrientation } from '../../../../../../types/tab-strip-types';
import { GROUP_COLORS } from '../../../../../../utils/tab-strip-internals';
import { TabButton } from '../../../tab-button/TabButton';
import { GroupHeader } from '../../../group-header/GroupHeader';

export function VerticalDragGroupVisual({
	group,
	collapsed,
	tabs,
	snapshot,
	width,
	collapsedStrip,
	touchUi,
	closeButtonAllowed,
}: {
	group: Readonly<TabGroupData>;
	collapsed: boolean;
	tabs: readonly Readonly<TabData>[];
	snapshot: TabModelSnapshot;
	width: number;
	collapsedStrip: boolean;
	touchUi: boolean;
	closeButtonAllowed: boolean;
}): ReactNode {
	const presentedGroup =
		group.collapsed === collapsed ? group : Object.freeze({ ...group, collapsed });
	const renderTab = (tab: Readonly<TabData>) => (
		<div key={tab.id} className={styles['ct-vertical-tab-wrap']} data-tab-strip-tab-wrapper=''>
			<TabButton
				tab={tab}
				snapshot={snapshot}
				width={Math.max(0, width - (collapsedStrip ? 12 : 10))}
				orientation={TabStripOrientation.Vertical}
				collapsed={collapsedStrip}
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
		</div>
	);
	const contents = tabs.map((tab) => renderTab(tab));
	return (
		<section
			className={styles['ct-vertical-group']}
			data-tab-strip-group=''
			data-group-id={group.id}
			data-group-collapsed={collapsed || undefined}
			style={
				{
					'--ob-comp-tab-strip-inline-size': `${width}px`,
					'--ob-comp-tab-strip-group-color': GROUP_COLORS[group.color],
				} satisfies CSSProperties
			}
		>
			<GroupHeader
				group={presentedGroup}
				tabs={tabs}
				activeTabId={snapshot.activeTabId}
				orientation={TabStripOrientation.Vertical}
				collapsedStrip={collapsedStrip}
				onToggle={() => undefined}
				onPointerDown={() => undefined}
				onKeyDown={() => undefined}
				onContextMenu={() => undefined}
				onOpenEditor={() => undefined}
				onMouseEnter={() => undefined}
				onMouseLeave={() => undefined}
				showHoverCard={false}
			/>
			<div
				className={styles['ct-vertical-group__tabs']}
				data-tab-strip-group-tabs=''
				aria-hidden={collapsed || undefined}
			>
				<div className={styles['ct-vertical-group__tabs-inner']}>{contents}</div>
			</div>
		</section>
	);
}
