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
	type TabStripNewTabSection,
	type TabStripNewTabRequest,
	TabStripOrientation,
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
	readonly newTabSections?: readonly TabStripNewTabSection[] | undefined;
	readonly onNewTerminal: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly onCloseTabs?: TabStripCloseCallback | undefined;
	readonly focusRequest: number;
	readonly getTabCreationAvailability?:
		| ((intent: TabStripTabCreationIntent) => ActionAvailability)
		| undefined;
	/**
	 * The mobile layout's Tab list: a vertical strip at the host's full width without its own
	 * controls or New Tab button, which the list leads with instead. Otherwise the strip is
	 * horizontal; the person does not choose its direction.
	 */
	readonly fill?: boolean | undefined;
	readonly onDraggingChange?: ((dragging: boolean) => void) | undefined;
	readonly onSelectTab?: TabStripSelectCallback | undefined;
	readonly onSetTabTitle: (tabId: string) => void;
	readonly onTabsChange: TabStripChangeCallback | undefined;
	readonly reserveSidebarToggleSpace: boolean;
	readonly tabIdentityIcons?: ReadonlyMap<string, ReactNode> | undefined;
	readonly tabModel: TabModel;
	readonly statusAdornments?: ReadonlyMap<string, Readonly<TabStatusAdornment>> | undefined;
	readonly tabStripMessages: TabStripMessages;
	/** Touch rows: 44 px targets 2 px apart. */
	readonly touchUi?: boolean | undefined;
}

export function WorkbenchTabBar({
	actionRuntime,
	closeResume,
	fill = false,
	newTabSections,
	onNewTerminal,
	onCloseTabs,
	onDraggingChange,
	focusRequest,
	getTabCreationAvailability,
	onSelectTab,
	onSetTabTitle,
	onTabsChange,
	parentActionScope,
	reserveSidebarToggleSpace,
	tabIdentityIcons,
	tabModel,
	statusAdornments,
	tabStripMessages,
	touchUi = false,
}: WorkbenchTabBarProps): ReactNode {
	const groupName = useTabGroupNameDialog(tabModel);
	const orientation = fill ? TabStripOrientation.Vertical : TabStripOrientation.Horizontal;
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
					// A desktop window's own controls at the top end, where they draw over the page.
					horizontalTrailingActions={
						<div className={styles['trailingActions']} data-slot='workbench-tab-bar-trailing' />
					}
					horizontalLeadingActions={
						reserveSidebarToggleSpace ? (
							<div
								className={styles['leadingActions']}
								data-sidebar-toggle-place=''
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
					onNameGroup={groupName.open}
					onSelectTab={onSelectTab}
					onSetTabTitle={onSetTabTitle}
					onTabsChange={onTabsChange}
					orientation={orientation}
					parentActionScope={parentActionScope}
					showGroupHoverCards={false}
					showHoverCardMemoryUsage={false}
					showHoverCardPreviews={false}
					newTabSections={newTabSections}
					showNewTabButton={!fill}
					touchUi={touchUi}
					verticalFill={fill}
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
