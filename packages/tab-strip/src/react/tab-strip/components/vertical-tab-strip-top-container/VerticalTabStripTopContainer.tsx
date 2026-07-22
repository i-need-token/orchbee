import type { CSSProperties, MouseEvent as ReactMouseEvent, ReactNode } from 'react';
import { TabStripOrientation } from '../../../../common';
import { getComboButtonFlatEdges, TAB_STRIP_FLAT_EDGE_BUTTON_SIZE } from '../../../../shared';
import { calculateVerticalTopContainerLayout } from '../../../../vertical';
import { FlatEdgeButton } from '../flat-edge-button/FlatEdgeButton';
import { TabStripIcon } from '../tab-strip-icon/TabStripIcon';
export interface VerticalTabStripTopContainerProps {
	availableWidth: number;
	collapsed: boolean;
	toolbarHeight?: number;
	captionButtonWidth?: number;
	isExitingExpandOnHover?: boolean;
	showCollapseButton?: boolean;
	showUnfocusButton?: boolean;
	showProjectsButton?: boolean;
	showTabSearchButton?: boolean;
	projectsTargetVisible?: boolean;
	tabSearchTargetVisible?: boolean;
	projectsAnimating?: boolean;
	tabSearchAnimating?: boolean;
	projectsOpen?: boolean;
	startButtonLabel?: string;
	onCollapsedChange?: (collapsed: boolean) => void;
	onUnfocusGroup?: () => void;
	onProjectsClick?: () => void;
	onTabSearchClick?: () => void;
	onProjectsContextMenu?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
	onTabSearchContextMenu?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
	onCollapseContextMenu?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
}

/** DOM view for Chromium's vertical collapse/unfocus and combo-button container. */
export function VerticalTabStripTopContainer({
	availableWidth,
	collapsed,
	toolbarHeight = 0,
	captionButtonWidth = 0,
	isExitingExpandOnHover = false,
	showCollapseButton = true,
	showUnfocusButton = false,
	showProjectsButton = false,
	showTabSearchButton = false,
	projectsTargetVisible = showProjectsButton,
	tabSearchTargetVisible = showTabSearchButton,
	projectsAnimating = false,
	tabSearchAnimating = false,
	projectsOpen = false,
	startButtonLabel = 'Projects',
	onCollapsedChange,
	onUnfocusGroup,
	onProjectsClick,
	onTabSearchClick,
	onProjectsContextMenu,
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
		comboStartVisible: showProjectsButton,
		comboEndVisible: showTabSearchButton,
	});
	const bothComboButtons = showProjectsButton && showTabSearchButton;
	const edges = getComboButtonFlatEdges(layout.orientation, bothComboButtons);
	const comboBounds = layout.children.find((child) => child.id === 'combo')?.bounds;
	const childStyle = (id: 'collapse' | 'unfocus' | 'combo'): CSSProperties => {
		const bounds = layout.children.find((child) => child.id === id)?.bounds;
		if (!bounds)
			return {
				display: 'none',
			};
		return {
			left: bounds.x,
			top: bounds.y,
			width: bounds.width,
			height: bounds.height,
		};
	};
	const showComboLabels =
		!collapsed &&
		layout.orientation === TabStripOrientation.Vertical &&
		(comboBounds?.width ?? 0) > TAB_STRIP_FLAT_EDGE_BUTTON_SIZE;
	return (
		<div
			className='ct-vertical-top-container'
			data-combo-orientation={layout.orientation}
			data-wrapped={layout.wrappedDueToOverflow || undefined}
			style={{
				height: layout.hostSize.height,
			}}
		>
			{showUnfocusButton ? (
				<button
					type='button'
					className='ct-vertical-top-button'
					style={childStyle('unfocus')}
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
					style={childStyle('collapse')}
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
			{showProjectsButton || showTabSearchButton ? (
				<div className='ct-vertical-top-combo' style={childStyle('combo')}>
					{showProjectsButton ? (
						<FlatEdgeButton
							type='button'
							icon={<TabStripIcon name='projects' />}
							label={startButtonLabel}
							showLabel={showComboLabels}
							aria-label={startButtonLabel}
							aria-pressed={projectsOpen}
							data-target-visible={projectsTargetVisible || undefined}
							data-animating={projectsAnimating || undefined}
							flatEdge={edges.start}
							expansionOrientation={layout.orientation}
							onClick={onProjectsClick}
							onContextMenu={onProjectsContextMenu}
						/>
					) : null}
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
