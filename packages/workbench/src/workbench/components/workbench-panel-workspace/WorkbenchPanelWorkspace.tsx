import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import type { TabModel } from '@orchbee/tab-strip';
import * as React from 'react';

import { WorkbenchPanelCloseDialog } from './components/workbench-panel-close-dialog/WorkbenchPanelCloseDialog';
import { WorkbenchPanelDragGhost } from './components/workbench-panel-drag-ghost/WorkbenchPanelDragGhost';
import { WorkbenchPanelLayout } from './components/workbench-panel-layout/WorkbenchPanelLayout';
import { useWorkbenchPanelWorkspace } from './hooks/useWorkbenchPanelWorkspace';
import styles from './WorkbenchPanelWorkspace.module.css';

interface WorkbenchPanelWorkspaceProps {
	readonly actionRuntime: ActionRuntime;
	readonly parentActionScope: ActionScope;
	readonly tabModel: TabModel;
}

export function WorkbenchPanelWorkspace({
	actionRuntime,
	parentActionScope,
	tabModel,
}: WorkbenchPanelWorkspaceProps): React.ReactNode {
	const view = useWorkbenchPanelWorkspace({ actionRuntime, parentActionScope, tabModel });

	return (
		<div
			className={styles['root']}
			data-dragging={view.panelDrag.draggingPanelId !== null || undefined}
			data-slot='workbench-panel-workspace'
		>
			{view.displayedRoot !== null && (
				<WorkbenchPanelLayout node={view.displayedRoot} view={view} />
			)}
			{view.panelDrag.ghost !== null && <WorkbenchPanelDragGhost ghost={view.panelDrag.ghost} />}
			<WorkbenchPanelCloseDialog view={view} />
		</div>
	);
}
