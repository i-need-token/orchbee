import type {
	ActionAvailability,
	ActionExecution,
	ActionRuntime,
	ActionScope,
} from '@orchbee/actions';
import {
	TabStrip,
	type TabModel,
	type TabStripChangeCallback,
	type TabStripCloseCallback,
	type TabStripCloseResume,
	type TabStripMessages,
	type TabStripNewTabChoice,
	type TabStripNewTabRequest,
	type TabStripOrientation,
	type TabStripSelectCallback,
	type TabStatusAdornment,
	type TabStripTabCreationIntent,
} from '@orchbee/tab-strip';
import type { ReactNode } from 'react';

import { NameDialog } from '../name-dialog/NameDialog';
import { useTabGroupNameDialog } from './hooks/useTabGroupNameDialog';
import styles from './WorkbenchTabBar.module.css';

interface WorkbenchTabBarProps {
	readonly actionRuntime: ActionRuntime;
	readonly parentActionScope: ActionScope;
	readonly closeResume?: TabStripCloseResume | undefined;
	readonly newTabChoices?: readonly TabStripNewTabChoice[] | undefined;
	readonly onNewTerminal: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly onCloseTabs?: TabStripCloseCallback | undefined;
	readonly focusRequest: number;
	readonly getTabCreationAvailability?:
		| ((intent: TabStripTabCreationIntent) => ActionAvailability)
		| undefined;
	/**
	 * The mobile layout's Tab list: a vertical strip at the host's full width without its own
	 * controls or New Tab button, which the list leads with instead.
	 */
	readonly fill?: boolean | undefined;
	readonly onDraggingChange?: ((dragging: boolean) => void) | undefined;
	/** Absent, the strip offers no orientation change. */
	readonly onOrientationChange?: ((orientation: TabStripOrientation) => void) | undefined;
	readonly onSelectTab?: TabStripSelectCallback | undefined;
	readonly onSetTabTitle: (tabId: string) => void;
	readonly onTabsChange: TabStripChangeCallback | undefined;
	readonly onVerticalCollapsedChange: (collapsed: boolean) => void;
	readonly orientation: TabStripOrientation;
	readonly reserveSidebarToggleSpace: boolean;
	readonly tabIdentityIcons?: ReadonlyMap<string, ReactNode> | undefined;
	readonly tabModel: TabModel;
	readonly statusAdornments?: ReadonlyMap<string, Readonly<TabStatusAdornment>> | undefined;
	readonly tabStripMessages: TabStripMessages;
	/** Touch rows: 44 px targets 2 px apart. */
	readonly touchUi?: boolean | undefined;
	readonly verticalCollapsed: boolean;
}

export function WorkbenchTabBar({
	actionRuntime,
	closeResume,
	fill = false,
	newTabChoices,
	onNewTerminal,
	onCloseTabs,
	onDraggingChange,
	focusRequest,
	getTabCreationAvailability,
	onOrientationChange,
	onSelectTab,
	onSetTabTitle,
	onTabsChange,
	onVerticalCollapsedChange,
	orientation,
	parentActionScope,
	reserveSidebarToggleSpace,
	tabIdentityIcons,
	tabModel,
	statusAdornments,
	tabStripMessages,
	touchUi = false,
	verticalCollapsed,
}: WorkbenchTabBarProps): ReactNode {
	const groupName = useTabGroupNameDialog(tabModel);
	return (
		<>
			<div
				className={styles['root']}
				data-slot='workbench-tab-bar'
				data-workbench-focus-region='tab-strip'
				data-fill={fill || undefined}
				data-orientation={orientation}
				data-reserve-sidebar-toggle-space={reserveSidebarToggleSpace || undefined}
			>
				<TabStrip
					actionRuntime={actionRuntime}
					focusRequest={focusRequest}
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
					identityIcons={tabIdentityIcons}
					statusAdornments={statusAdornments}
					onNewTab={onNewTerminal}
					onCloseTabs={onCloseTabs}
					onDraggingChange={onDraggingChange}
					closeResume={closeResume}
					getTabCreationAvailability={getTabCreationAvailability}
					onOrientationChange={onOrientationChange}
					onNameGroup={groupName.open}
					onSelectTab={onSelectTab}
					onSetTabTitle={onSetTabTitle}
					onTabsChange={onTabsChange}
					orientation={orientation}
					parentActionScope={parentActionScope}
					showGroupHoverCards={false}
					showHoverCardMemoryUsage={false}
					showHoverCardPreviews={false}
					newTabChoices={newTabChoices}
					showNewTabButton={!fill}
					showVerticalTopControls={!fill}
					touchUi={touchUi}
					verticalCollapsed={!fill && verticalCollapsed}
					verticalFill={fill}
					onVerticalCollapsedChange={onVerticalCollapsedChange}
				/>
			</div>
			{groupName.dialog === null ? null : (
				<NameDialog
					blank={groupName.dialog.blank}
					messages={groupName.dialog.messages}
					name={groupName.dialog.name}
					onClose={groupName.dialog.close}
					onNameChange={groupName.dialog.setName}
					onSubmit={groupName.dialog.submit}
					submitLabel={groupName.dialog.submitLabel}
					title={groupName.dialog.title}
				/>
			)}
		</>
	);
}
