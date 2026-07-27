import type { ActionExecution, ActionRuntime, ActionScope } from '@orchbee/actions';
import {
	TabStrip,
	type TabModel,
	type TabStripCrossWindowDragRuntimeFactory,
	type TabStripMessages,
	type TabStripNewTabRequest,
	type TabStripOrientation,
} from '@orchbee/tab-strip';
import type { ReactNode } from 'react';

import styles from './WorkbenchTabBar.module.css';

interface WorkbenchTabBarProps {
	readonly actionRuntime: ActionRuntime;
	readonly parentActionScope: ActionScope;
	readonly createCrossWindowDragRuntime: TabStripCrossWindowDragRuntimeFactory | undefined;
	readonly onNewTerminal: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly onOrientationChange: (orientation: TabStripOrientation) => void;
	readonly onVerticalCollapsedChange: (collapsed: boolean) => void;
	readonly orientation: TabStripOrientation;
	readonly reserveSidebarToggleSpace: boolean;
	readonly tabModel: TabModel;
	readonly tabStripMessages: TabStripMessages;
	readonly verticalCollapsed: boolean;
}

export function WorkbenchTabBar({
	actionRuntime,
	createCrossWindowDragRuntime,
	onNewTerminal,
	onOrientationChange,
	onVerticalCollapsedChange,
	orientation,
	parentActionScope,
	reserveSidebarToggleSpace,
	tabModel,
	tabStripMessages,
	verticalCollapsed,
}: WorkbenchTabBarProps): ReactNode {
	return (
		<div
			className={styles['root']}
			data-slot='workbench-tab-bar'
			data-orientation={orientation}
			data-reserve-sidebar-toggle-space={reserveSidebarToggleSpace || undefined}
		>
			<TabStrip
				actionRuntime={actionRuntime}
				createCrossWindowDragRuntime={createCrossWindowDragRuntime}
				horizontalLeadingActions={
					reserveSidebarToggleSpace ? (
						<div
							className={styles['leadingActions']}
							data-slot='workbench-tab-bar-leading-actions'
						/>
					) : null
				}
				messages={tabStripMessages}
				model={tabModel}
				onNewTab={onNewTerminal}
				onOrientationChange={onOrientationChange}
				orientation={orientation}
				parentActionScope={parentActionScope}
				showGroupHoverCards={false}
				showHoverCardMemoryUsage={false}
				showHoverCardPreviews={false}
				verticalCollapsed={verticalCollapsed}
				onVerticalCollapsedChange={onVerticalCollapsedChange}
			/>
		</div>
	);
}
