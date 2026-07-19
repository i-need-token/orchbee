import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@orchbee/ui/components';
import * as React from 'react';

import {
	MAXIMUM_LEFT_PANEL_WIDTH,
	MINIMUM_LEFT_PANEL_WIDTH,
	type WorkbenchLayoutStorage,
} from '../workbench-layout-storage';
import { useWorkbenchShellLayout } from './hooks/useWorkbenchShellLayout';
import styles from './WorkbenchShell.module.css';

interface WorkbenchShellProps {
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
}

export function WorkbenchShell({ layoutStorage }: WorkbenchShellProps): React.ReactNode {
	const layout = useWorkbenchShellLayout(layoutStorage);

	return (
		<div className={styles['root']}>
			<div className={styles['layout']}>
				<ResizablePanelGroup orientation='horizontal' onLayoutChanged={layout.onLayoutChanged}>
					{/* Keep the navigation panel on an explicit pixel contract. */}
					<ResizablePanel
						id='left'
						defaultSize={layout.defaultLeftPanelSize}
						minSize={`${MINIMUM_LEFT_PANEL_WIDTH}px`}
						maxSize={`${MAXIMUM_LEFT_PANEL_WIDTH}px`}
						groupResizeBehavior='preserve-pixel-size'
						onResize={layout.onLeftPanelResize}
					>
						<div className={styles['leftPanel']} data-slot='workbench-left-panel' />
					</ResizablePanel>
					<ResizableHandle className={styles['resizeHandle']} />
					<ResizablePanel id='right'>
						<div className={styles['rightPanel']} data-slot='workbench-right-panel' />
					</ResizablePanel>
				</ResizablePanelGroup>
			</div>
		</div>
	);
}
