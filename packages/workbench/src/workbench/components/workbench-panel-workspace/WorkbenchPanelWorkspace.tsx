import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import * as React from 'react';

import type { PanelWorkspaceStore } from './model/PanelWorkspaceStore';
import type { WorkbenchTerminalSurface } from '../../types/workbench-terminal-surface';
import { WorkbenchPanelCloseDialog } from './components/workbench-panel-close-dialog/WorkbenchPanelCloseDialog';
import { WorkbenchPanelDragGhost } from './components/workbench-panel-drag-ghost/WorkbenchPanelDragGhost';
import { WorkbenchPanelLayout } from './components/workbench-panel-layout/WorkbenchPanelLayout';
import { useWorkbenchPanelWorkspace } from './hooks/useWorkbenchPanelWorkspace';
import { useTerminalTabPresentationHandoff } from './hooks/useTerminalTabPresentationHandoff';
import { projectTerminalTabPresentationLayers } from './model/projectTerminalTabPresentationLayers';
import { projectWorkbenchPanelWorkspaceTabView } from './model/projectWorkbenchPanelWorkspaceTabView';
import type { TerminalTabPresentationLayer } from './types/terminal-tab-presentation-layer';
import styles from './WorkbenchPanelWorkspace.module.css';

interface WorkbenchPanelWorkspaceProps {
	readonly actionRuntime: ActionRuntime;
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly parentActionScope: ActionScope;
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
}

export function WorkbenchPanelWorkspace({
	actionRuntime,
	panelWorkspaceStore,
	parentActionScope,
	shortcutLabelForAction,
	terminalSurface,
}: WorkbenchPanelWorkspaceProps): React.ReactNode {
	const view = useWorkbenchPanelWorkspace({
		actionRuntime,
		panelWorkspaceStore,
		parentActionScope,
		shortcutLabelForAction,
	});
	const handoff = useTerminalTabPresentationHandoff({
		activeTab: view.activeTab,
		tabs: view.state.tabs,
		terminalSurfaceAvailable: terminalSurface !== undefined,
	});
	const presentationLayers = projectTerminalTabPresentationLayers(view.state.tabs, handoff.state);

	return (
		<div
			className={styles['root']}
			data-dragging={view.panelDrag.draggingPanelId !== null || undefined}
			data-slot='workbench-panel-workspace'
		>
			{presentationLayers.map(({ epoch, mode, tab }) => {
				const layer: TerminalTabPresentationLayer = {
					epoch,
					mode,
					reportPresentationReady: (presentationId, readiness) => {
						handoff.reportPresentationReady(epoch, presentationId, readiness);
					},
				};
				const layerView = projectWorkbenchPanelWorkspaceTabView(view, tab);
				const node = mode === 'active' ? (view.panelDrag.previewRoot ?? tab.root) : tab.root;
				return (
					<div
						aria-hidden={mode === 'active' ? undefined : true}
						className={styles['presentationLayer']}
						data-presentation-mode={mode}
						data-terminal-tab-id={tab.tabId}
						inert={mode === 'active' ? undefined : true}
						key={tab.tabId}
					>
						<WorkbenchPanelLayout
							layer={layer}
							node={node}
							terminalSurface={terminalSurface}
							view={layerView}
						/>
					</div>
				);
			})}
			{view.panelDrag.ghost !== null && <WorkbenchPanelDragGhost ghost={view.panelDrag.ghost} />}
			<WorkbenchPanelCloseDialog view={view} />
		</div>
	);
}
