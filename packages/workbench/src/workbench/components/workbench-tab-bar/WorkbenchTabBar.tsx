import type { ActionExecution, ActionRuntime, ActionScope } from '@orchbee/actions';
import {
	TabStrip,
	type TabModel,
	type TabStripCrossWindowDragRuntimeFactory,
	type TabStripCloseCallback,
	type TabStripCloseConfirmation,
	type TabStripMessages,
	type TabStripMoveGroupToWindowCallback,
	type TabStripMoveTabsToWindowCallback,
	type TabStripNewTabRequest,
	type TabStripOrientation,
	type TabStripSelectCallback,
	type TabStripShortcutLabels,
	type TabStripWindowTarget,
} from '@orchbee/tab-strip';
import type { ReactNode } from 'react';

import styles from './WorkbenchTabBar.module.css';

interface WorkbenchTabBarProps {
	readonly actionRuntime: ActionRuntime;
	readonly parentActionScope: ActionScope;
	readonly createCrossWindowDragRuntime: TabStripCrossWindowDragRuntimeFactory | undefined;
	readonly onNewTerminal: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly onMoveGroupToWindow?: TabStripMoveGroupToWindowCallback | undefined;
	readonly onMoveTabsToWindow?: TabStripMoveTabsToWindowCallback | undefined;
	readonly onCloseTabs?: TabStripCloseCallback | undefined;
	readonly getCloseTabsConfirmation: TabStripCloseConfirmation;
	readonly onOrientationChange: (orientation: TabStripOrientation) => void;
	readonly onSelectTab?: TabStripSelectCallback | undefined;
	readonly onSetTabTitle: (tabId: string) => void;
	readonly onVerticalCollapsedChange: (collapsed: boolean) => void;
	readonly orientation: TabStripOrientation;
	readonly reserveSidebarToggleSpace: boolean;
	readonly shortcutLabels: TabStripShortcutLabels;
	readonly tabModel: TabModel;
	readonly tabStripMessages: TabStripMessages;
	readonly verticalCollapsed: boolean;
	readonly windowTargets?: readonly TabStripWindowTarget[] | undefined;
}

export function WorkbenchTabBar({
	actionRuntime,
	createCrossWindowDragRuntime,
	onNewTerminal,
	onMoveGroupToWindow,
	onMoveTabsToWindow,
	onCloseTabs,
	getCloseTabsConfirmation,
	onOrientationChange,
	onSelectTab,
	onSetTabTitle,
	onVerticalCollapsedChange,
	orientation,
	parentActionScope,
	reserveSidebarToggleSpace,
	shortcutLabels,
	tabModel,
	tabStripMessages,
	verticalCollapsed,
	windowTargets = [],
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
				onMoveGroupToWindow={onMoveGroupToWindow}
				onMoveTabsToWindow={onMoveTabsToWindow}
				onCloseTabs={onCloseTabs}
				getCloseTabsConfirmation={getCloseTabsConfirmation}
				onOrientationChange={onOrientationChange}
				onSelectTab={onSelectTab}
				onSetTabTitle={onSetTabTitle}
				orientation={orientation}
				parentActionScope={parentActionScope}
				showGroupHoverCards={false}
				showHoverCardMemoryUsage={false}
				showHoverCardPreviews={false}
				shortcutLabels={shortcutLabels}
				verticalCollapsed={verticalCollapsed}
				onVerticalCollapsedChange={onVerticalCollapsedChange}
				windowTargets={windowTargets}
			/>
		</div>
	);
}
