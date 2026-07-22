import type { MouseEvent as ReactMouseEvent, ReactNode } from 'react';
import { TabStripOrientation } from '../../../../common';
import { getComboButtonFlatEdges, TAB_STRIP_FLAT_EDGE_BUTTON_SIZE } from '../../../../shared';
import { calculateVerticalTopContainerLayout } from '../../../../vertical';
import { TabStripIcon } from '../tab-strip-icon/TabStripIcon';
import { FlatEdgeButton } from '../flat-edge-button/FlatEdgeButton';
import { getVerticalTopContainerChildStyle } from './utils/getVerticalTopContainerChildStyle';

export interface VerticalTabStripTopContainerProps {
	availableWidth: number;
	collapsed: boolean;
	toolbarHeight?: number;
	captionButtonWidth?: number;
	isExitingExpandOnHover?: boolean;
	showCollapseButton?: boolean;
	showUnfocusButton?: boolean;
	showTabSearchButton?: boolean;
	tabSearchTargetVisible?: boolean;
	tabSearchAnimating?: boolean;
	onCollapsedChange?: (collapsed: boolean) => void;
	onUnfocusGroup?: () => void;
	onTabSearchClick?: () => void;
	onTabSearchContextMenu?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
	onCollapseContextMenu?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
}

/** DOM view for the vertical collapse, unfocus, and tab-search controls. */
export function VerticalTabStripTopContainer({
	availableWidth,
	collapsed,
	toolbarHeight = 0,
	captionButtonWidth = 0,
	isExitingExpandOnHover = false,
	showCollapseButton = true,
	showUnfocusButton = false,
	showTabSearchButton = false,
	tabSearchTargetVisible = showTabSearchButton,
	tabSearchAnimating = false,
	onCollapsedChange,
	onUnfocusGroup,
	onTabSearchClick,
	onTabSearchContextMenu,
	onCollapseContextMenu,
}: VerticalTabStripTopContainerProps): ReactNode {
	const layout = calculateVerticalTopContainerLayout({
		availableWidth,
		toolbarHeight,
		captionButtonWidth,
		collapsed,
		isExitingExpandOnHover,
		collapseVisible: showCollapseButton,
		unfocusVisible: showUnfocusButton,
		comboEndVisible: showTabSearchButton,
	});
	const edges = getComboButtonFlatEdges(layout.orientation, false);
	const comboBounds = layout.children.find((child) => child.id === 'combo')?.bounds;
	const showComboLabels =
		!collapsed &&
		layout.orientation === TabStripOrientation.Vertical &&
		(comboBounds?.width ?? 0) > TAB_STRIP_FLAT_EDGE_BUTTON_SIZE;

	return (
		<div
			className='ct-vertical-top-container'
			data-combo-orientation={layout.orientation}
			data-wrapped={layout.wrappedDueToOverflow || undefined}
			style={{ height: layout.hostSize.height }}
		>
			{showUnfocusButton ? (
				<button
					type='button'
					className='ct-vertical-top-button'
					style={getVerticalTopContainerChildStyle(layout, 'unfocus')}
					aria-label='Show all tabs'
					onClick={onUnfocusGroup}
				>
					<TabStripIcon name='unfocus-group' />
				</button>
			) : null}
			{showCollapseButton ? (
				<button
					type='button'
					className='ct-vertical-top-button'
					style={getVerticalTopContainerChildStyle(layout, 'collapse')}
					aria-label={collapsed ? 'Expand tab strip' : 'Collapse tab strip'}
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
					className='ct-vertical-top-combo'
					style={getVerticalTopContainerChildStyle(layout, 'combo')}
				>
					{showTabSearchButton ? (
						<FlatEdgeButton
							type='button'
							icon={<TabStripIcon name='search' />}
							label='Search tabs'
							showLabel={showComboLabels}
							aria-label='Search tabs'
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
