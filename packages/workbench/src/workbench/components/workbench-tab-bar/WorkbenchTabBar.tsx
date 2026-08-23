import type { ActionExecution, ActionRuntime, ActionScope } from '@orchbee/actions';
import {
	TabStrip,
	type TabModel,
	type TabStripCrossWindowDragRuntimeFactory,
	type TabStripCloseCallback,
	type TabStripCloseConfirmation,
	type TabStripMessages,
	type TabStripNewTabRequest,
	type TabStripOrientation,
	type TabStripSelectCallback,
	type TabStripShortcutLabels,
} from '@orchbee/tab-strip';
import type { ReactNode } from 'react';

import styles from './WorkbenchTabBar.module.css';

interface WorkbenchTabBarProps {
	readonly actionRuntime: ActionRuntime;
	readonly parentActionScope: ActionScope;
	readonly createCrossWindowDragRuntime: TabStripCrossWindowDragRuntimeFactory | undefined;
	readonly onNewTerminal: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly onCloseTabs?: TabStripCloseCallback | undefined;
	readonly getCloseTabsConfirmation: TabStripCloseConfirmation;
	readonly onOrientationChange: (orientation: TabStripOrientation) => void;
	readonly onSelectTab?: TabStripSelectCallback | undefined;
	readonly onVerticalCollapsedChange: (collapsed: boolean) => void;
	readonly orientation: TabStripOrientation;
	readonly reserveSidebarToggleSpace: boolean;
	readonly shortcutLabels: TabStripShortcutLabels;
	readonly tabModel: TabModel;
	readonly tabStripMessages: TabStripMessages;
	readonly verticalCollapsed: boolean;
}

export function WorkbenchTabBar({
	actionRuntime,
	createCrossWindowDragRuntime,
	onNewTerminal,
	onCloseTabs,
	getCloseTabsConfirmation,
	onOrientationChange,
	onSelectTab,
	onVerticalCollapsedChange,
	orientation,
	parentActionScope,
	reserveSidebarToggleSpace,
	shortcutLabels,
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
				onCloseTabs={onCloseTabs}
				getCloseTabsConfirmation={getCloseTabsConfirmation}
				onOrientationChange={onOrientationChange}
				onSelectTab={onSelectTab}
				orientation={orientation}
				parentActionScope={parentActionScope}
				showGroupHoverCards={false}
				showHoverCardMemoryUsage={false}
				showHoverCardPreviews={false}
				shortcutLabels={shortcutLabels}
				verticalCollapsed={verticalCollapsed}
				onVerticalCollapsedChange={onVerticalCollapsedChange}
			/>
		</div>
	);
}
