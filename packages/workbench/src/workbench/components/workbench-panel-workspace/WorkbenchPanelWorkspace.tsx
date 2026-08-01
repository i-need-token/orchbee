import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import * as React from 'react';

import type { PanelWorkspaceStore } from './model/PanelWorkspaceStore';
import { WorkbenchPanelCloseDialog } from './components/workbench-panel-close-dialog/WorkbenchPanelCloseDialog';
import { WorkbenchPanelDragGhost } from './components/workbench-panel-drag-ghost/WorkbenchPanelDragGhost';
import { WorkbenchPanelLayout } from './components/workbench-panel-layout/WorkbenchPanelLayout';
import { useWorkbenchPanelWorkspace } from './hooks/useWorkbenchPanelWorkspace';
import styles from './WorkbenchPanelWorkspace.module.css';

interface WorkbenchPanelWorkspaceProps {
	readonly actionRuntime: ActionRuntime;
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly parentActionScope: ActionScope;
}

export function WorkbenchPanelWorkspace({
	actionRuntime,
	panelWorkspaceStore,
	parentActionScope,
}: WorkbenchPanelWorkspaceProps): React.ReactNode {
	const view = useWorkbenchPanelWorkspace({
		actionRuntime,
		panelWorkspaceStore,
		parentActionScope,
	});

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
