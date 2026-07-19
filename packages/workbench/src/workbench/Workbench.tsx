import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@orchbee/ui/components';
import * as React from 'react';

import {
	MAXIMUM_LEFT_PANEL_WIDTH,
	MINIMUM_LEFT_PANEL_WIDTH,
	type WorkbenchLayoutStorage,
} from '../workbench-layout-storage';
import { WorkbenchSidebarToggle } from './components/workbench-sidebar-toggle/WorkbenchSidebarToggle';
import { WorkbenchTabs } from './components/workbench-tabs/WorkbenchTabs';
import { useWorkbench } from './hooks/useWorkbench';
import type { WorkbenchShortcutPlatform } from './types/workbench-shortcut-platform';
import type { WorkbenchTab } from './types/workbench-tab';
import styles from './Workbench.module.css';

const EMPTY_WORKBENCH_TABS: readonly WorkbenchTab[] = [];

export interface WorkbenchProps {
	readonly activeTabId?: string | undefined;
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly onActiveTabChange?: ((tabId: string) => void) | undefined;
	readonly onTabClose?: ((tabId: string) => void) | undefined;
	readonly onTabCreate?: (() => void) | undefined;
	readonly onTabMove?: ((tabId: string, targetIndex: number) => void) | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly tabs?: readonly WorkbenchTab[] | undefined;
}

export function Workbench({
	activeTabId,
	layoutStorage,
	onActiveTabChange,
	onTabClose,
	onTabCreate,
	onTabMove,
	shortcutPlatform,
	tabs = EMPTY_WORKBENCH_TABS,
}: WorkbenchProps): React.ReactNode {
	const view = useWorkbench(layoutStorage, shortcutPlatform);
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
								<div className={styles['leftPanel']} data-slot='workbench-left-panel' />
							</ResizablePanel>
							<ResizableHandle className={styles['resizeHandle']} />
						</>
					)}
					<ResizablePanel id='right'>
						<div className={styles['rightPanel']} data-slot='workbench-right-panel'>
							<WorkbenchTabs
								activeTabId={activeTabId}
								ariaLabel={view.tabListLabel}
								onActiveTabChange={onActiveTabChange}
								onTabClose={onTabClose}
								onTabCreate={onTabCreate}
								onTabMove={onTabMove}
								reserveLeadingControlSpace={!view.isSidebarVisible}
								tabs={tabs}
							/>
						</div>
					</ResizablePanel>
				</ResizablePanelGroup>
			</div>
		</div>
	);
}
