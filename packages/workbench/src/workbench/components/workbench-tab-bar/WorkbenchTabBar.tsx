import { TabStrip, type TabModel, type TabStripMessages } from '@orchbee/tab-strip';
import * as React from 'react';

import styles from './WorkbenchTabBar.module.css';

interface WorkbenchTabBarProps {
	readonly onNewTerminal: (() => void) | undefined;
	readonly reserveSidebarToggleSpace: boolean;
	readonly tabModel: TabModel;
	readonly tabStripMessages: TabStripMessages;
}

export function WorkbenchTabBar({
	onNewTerminal,
	reserveSidebarToggleSpace,
	tabModel,
	tabStripMessages,
}: WorkbenchTabBarProps): React.ReactNode {
	return (
		<div className={styles['root']} data-slot='workbench-tab-bar'>
			<TabStrip
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
				showGroupHoverCards={false}
				showHoverCardMemoryUsage={false}
				showHoverCardPreviews={false}
			/>
		</div>
	);
}
