import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@orchbee/ui/components';
import * as React from 'react';

import {
	MAXIMUM_LEFT_PANEL_WIDTH,
	MINIMUM_LEFT_PANEL_WIDTH,
	type WorkbenchLayoutStorage,
} from '../workbench-layout-storage';
import { WorkbenchSidebarToggle } from './components/workbench-sidebar-toggle/WorkbenchSidebarToggle';
import { WorkbenchSidebar } from './components/workbench-sidebar/WorkbenchSidebar';
import { useWorkbench } from './hooks/useWorkbench';
import type { WorkbenchShortcutPlatform } from './types/workbench-shortcut-platform';
import styles from './Workbench.module.css';

export interface WorkbenchProps {
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly onNewTerminal?: (() => void) | undefined;
	readonly onSearch?: (() => void) | undefined;
	readonly onSettings?: (() => void) | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
}

export function Workbench({
	layoutStorage,
	onNewTerminal,
	onSearch,
	onSettings,
	shortcutPlatform,
}: WorkbenchProps): React.ReactNode {
	const view = useWorkbench({
		onNewTerminal,
		onSearch,
		onSettings,
		shortcutPlatform,
		storage: layoutStorage,
	});
	const sidebarToggle = (
		<WorkbenchSidebarToggle
			isSidebarVisible={view.isSidebarVisible}
			label={view.sidebarToggleLabel}
			onToggle={view.toggleSidebar}
			shortcutId={view.sidebarShortcutId}
			shortcutLabel={view.sidebarShortcutLabel}
		/>
	);

	return (
		<div className={styles['root']}>
			<div className={styles['sidebarToggle']}>{sidebarToggle}</div>
			<div className={styles['layout']}>
				<ResizablePanelGroup orientation='horizontal' onLayoutChanged={view.onLayoutChanged}>
					{view.isSidebarVisible && (
						<>
							{/* Keep the navigation panel on an explicit pixel contract. */}
							<ResizablePanel
								id='left'
								defaultSize={view.leftPanelSize}
								minSize={`${MINIMUM_LEFT_PANEL_WIDTH}px`}
								maxSize={`${MAXIMUM_LEFT_PANEL_WIDTH}px`}
								groupResizeBehavior='preserve-pixel-size'
								onResize={view.onLeftPanelResize}
							>
								<div className={styles['leftPanel']} data-slot='workbench-left-panel'>
									<WorkbenchSidebar
										label={view.sidebarLabel}
										newTerminalLabel={view.newTerminalLabel}
										newTerminalShortcutId={view.newTerminalShortcutId}
										newTerminalShortcutLabel={view.newTerminalShortcutLabel}
										onNewTerminal={onNewTerminal}
										onSearch={onSearch}
										onSettings={onSettings}
										projectLabel={view.projectLabel}
										searchLabel={view.searchLabel}
										searchShortcutId={view.searchShortcutId}
										searchShortcutLabel={view.searchShortcutLabel}
										settingsLabel={view.settingsLabel}
										settingsShortcutId={view.settingsShortcutId}
										settingsShortcutLabel={view.settingsShortcutLabel}
									/>
								</div>
							</ResizablePanel>
							<ResizableHandle className={styles['resizeHandle']} />
						</>
					)}
					<ResizablePanel id='right'>
						<div className={styles['rightPanel']} data-slot='workbench-right-panel' />
					</ResizablePanel>
				</ResizablePanelGroup>
			</div>
		</div>
	);
}
