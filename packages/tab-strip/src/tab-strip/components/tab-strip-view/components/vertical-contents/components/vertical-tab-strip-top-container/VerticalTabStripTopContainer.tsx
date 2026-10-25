import styles from './VerticalTabStripTopContainer.module.css';
import type { MouseEvent as ReactMouseEvent, ReactNode } from 'react';
import { TabStripOrientation } from '../../../../../../types/tab-strip-types';
import {
	getComboButtonFlatEdges,
	TAB_STRIP_FLAT_EDGE_BUTTON_SIZE,
} from '../flat-edge-button/utils/tab-strip-flat-edge-button';
import { calculateVerticalTopContainerLayout } from './utils/calculateVerticalTopContainerLayout';
import { TabStripIcon } from '../../../tab-strip-icon/TabStripIcon';
import { FlatEdgeButton } from '../flat-edge-button/FlatEdgeButton';
import { getVerticalTopContainerChildBounds } from './utils/getVerticalTopContainerChildBounds';
import { useTabStripMessages } from '../../../../../../hooks/useTabStripMessages';

interface VerticalTabStripTopContainerProps {
	availableWidth: number;
	collapsed: boolean;
	toolbarHeight?: number;
	captionButtonWidth?: number;
	isExitingExpandOnHover?: boolean;
	showCollapseButton?: boolean;
	showTabSearchButton?: boolean;
	tabSearchTargetVisible?: boolean;
	tabSearchAnimating?: boolean;
	onCollapsedChange?: ((collapsed: boolean) => void) | undefined;
	onTabSearchClick?: (() => void) | undefined;
	onTabSearchContextMenu?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
	onCollapseContextMenu?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
}

/** DOM view for the vertical collapse and tab-search controls. */
export function VerticalTabStripTopContainer({
	availableWidth,
	collapsed,
	toolbarHeight = 0,
	captionButtonWidth = 0,
	isExitingExpandOnHover = false,
	showCollapseButton = true,
	showTabSearchButton = false,
	tabSearchTargetVisible = showTabSearchButton,
	tabSearchAnimating = false,
	onCollapsedChange,
	onTabSearchClick,
	onTabSearchContextMenu,
	onCollapseContextMenu,
}: VerticalTabStripTopContainerProps): ReactNode {
	const messages = useTabStripMessages();
	const layout = calculateVerticalTopContainerLayout({
		availableWidth,
		toolbarHeight,
		captionButtonWidth,
		collapsed,
		isExitingExpandOnHover,
		collapseVisible: showCollapseButton,
		comboEndVisible: showTabSearchButton,
	});
	const edges = getComboButtonFlatEdges(layout.orientation, false);
	const comboBounds = layout.children.find((child) => child.id === 'combo')?.bounds;
	const showComboLabels =
		!collapsed &&
		layout.orientation === TabStripOrientation.Vertical &&
		(comboBounds?.width ?? 0) > TAB_STRIP_FLAT_EDGE_BUTTON_SIZE;
	const collapseBounds = getVerticalTopContainerChildBounds(layout, 'collapse');
	const resolvedComboBounds = getVerticalTopContainerChildBounds(layout, 'combo');

	return (
		<div
			className={styles['ct-vertical-top-container']}
			data-vertical-top-container=''
			data-combo-orientation={layout.orientation}
			data-wrapped={layout.wrappedDueToOverflow || undefined}
			style={{ '--ob-comp-tab-strip-block-size': `${layout.hostSize.height}px` }}
		>
			{showCollapseButton ? (
				<button
					type='button'
					className={styles['ct-vertical-top-button']}
					hidden={collapseBounds === null}
					style={{
						'--ob-comp-tab-strip-inline-start': collapseBounds
							? `${collapseBounds.x}px`
							: undefined,
						'--ob-comp-tab-strip-block-start': collapseBounds ? `${collapseBounds.y}px` : undefined,
						'--ob-comp-tab-strip-inline-size': collapseBounds
							? `${collapseBounds.width}px`
							: undefined,
						'--ob-comp-tab-strip-block-size': collapseBounds
							? `${collapseBounds.height}px`
							: undefined,
					}}
					aria-label={collapsed ? messages.expandTabStrip : messages.collapseTabStrip}
					onClick={() => onCollapsedChange?.(!collapsed)}
					onContextMenu={onCollapseContextMenu}
				>
					{collapsed ? (
						<TabStripIcon name='expand-sidebar' />
					) : (
						<TabStripIcon name='collapse-sidebar' />
					)}
				</button>
			) : null}
			{showTabSearchButton ? (
				<div
					className={styles['ct-vertical-top-combo']}
					hidden={resolvedComboBounds === null}
					style={{
						'--ob-comp-tab-strip-inline-start': resolvedComboBounds
							? `${resolvedComboBounds.x}px`
							: undefined,
						'--ob-comp-tab-strip-block-start': resolvedComboBounds
							? `${resolvedComboBounds.y}px`
							: undefined,
						'--ob-comp-tab-strip-inline-size': resolvedComboBounds
							? `${resolvedComboBounds.width}px`
							: undefined,
						'--ob-comp-tab-strip-block-size': resolvedComboBounds
							? `${resolvedComboBounds.height}px`
							: undefined,
					}}
				>
					{showTabSearchButton ? (
						<FlatEdgeButton
							type='button'
							icon={<TabStripIcon name='search' />}
							label={messages.searchTabs}
							showLabel={showComboLabels}
							aria-label={messages.searchTabs}
							data-target-visible={tabSearchTargetVisible || undefined}
							data-animating={tabSearchAnimating || undefined}
							flatEdge={edges.end}
							expansionOrientation={layout.orientation}
							onClick={onTabSearchClick}
							onContextMenu={onTabSearchContextMenu}
						/>
					) : null}
				</div>
			) : null}
		</div>
	);
}
