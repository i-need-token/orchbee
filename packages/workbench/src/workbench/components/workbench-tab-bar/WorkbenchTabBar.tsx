import {
	TabStrip,
	type TabModel,
	type TabStripCrossWindowDragRuntimeFactory,
	type TabStripMessages,
	type TabStripOrientation,
} from '@orchbee/tab-strip';
import * as React from 'react';

import styles from './WorkbenchTabBar.module.css';

interface WorkbenchTabBarProps {
	readonly createCrossWindowDragRuntime: TabStripCrossWindowDragRuntimeFactory | undefined;
	readonly onNewTerminal: (() => void) | undefined;
	readonly onOrientationChange: (orientation: TabStripOrientation) => void;
	readonly orientation: TabStripOrientation;
	readonly reserveSidebarToggleSpace: boolean;
	readonly tabModel: TabModel;
	readonly tabStripMessages: TabStripMessages;
}

export function WorkbenchTabBar({
	createCrossWindowDragRuntime,
	onNewTerminal,
	onOrientationChange,
	orientation,
	reserveSidebarToggleSpace,
	tabModel,
	tabStripMessages,
}: WorkbenchTabBarProps): React.ReactNode {
	return (
		<div
			className={styles['root']}
			data-slot='workbench-tab-bar'
			data-orientation={orientation}
			data-reserve-sidebar-toggle-space={reserveSidebarToggleSpace || undefined}
		>
			<TabStrip
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
				showGroupHoverCards={false}
				showHoverCardMemoryUsage={false}
				showHoverCardPreviews={false}
			/>
		</div>
	);
}
