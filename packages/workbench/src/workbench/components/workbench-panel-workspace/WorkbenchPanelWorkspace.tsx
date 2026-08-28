import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import type { ReactNode } from 'react';

import type { PanelWorkspaceStore } from './model/PanelWorkspaceStore';
import type { WorkbenchTerminalSurface } from '../../types/workbench-terminal-surface';
import { WorkbenchPanelCloseDialog } from './components/workbench-panel-close-dialog/WorkbenchPanelCloseDialog';
import { WorkbenchPanelDragGhost } from './components/workbench-panel-drag-ghost/WorkbenchPanelDragGhost';
import { WorkbenchPanelLayout } from './components/workbench-panel-layout/WorkbenchPanelLayout';
import { WorkbenchPanelResizeDialog } from './components/workbench-panel-resize-dialog/WorkbenchPanelResizeDialog';
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
}: WorkbenchPanelWorkspaceProps): ReactNode {
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
	const presentationLayers = projectTerminalTabPresentationLayers(
		view.state.tabs,
		handoff.state,
		handoff.displayedTab,
	);

	return (
		<div
			aria-label={view.panelVisualSelectionLabel}
			className={styles['root']}
			data-dragging={view.panelDrag.draggingPanelId !== null || undefined}
			data-panel-visual-selection-mode={view.panelVisualSelection?.mode}
			data-slot='workbench-panel-workspace'
			data-workbench-keyboard-capture={view.panelVisualSelection === null ? undefined : 'true'}
			onBlurCapture={(event) => {
				if (
					view.panelVisualSelection !== null &&
					(!(event.relatedTarget instanceof Node) ||
						!event.currentTarget.contains(event.relatedTarget))
				) {
					view.cancelPanelVisualSelection(false);
				}
			}}
			onKeyDownCapture={view.handlePanelVisualSelectionKeyDown}
			onPointerDownCapture={view.handlePanelVisualSelectionPointerDown}
			ref={view.workspaceReference}
			tabIndex={-1}
		>
			{presentationLayers.map(({ epoch, mode, tab }) => {
				const layer: TerminalTabPresentationLayer = {
					epoch,
					mode,
					reportPresentationReady: (presentationId, readiness) => {
						handoff.reportPresentationReady(epoch, tab.tabId, presentationId, readiness);
					},
				};
				const layerView = projectWorkbenchPanelWorkspaceTabView(view, tab);
				const node = mode === 'active' ? (layerView.panelDrag.previewRoot ?? tab.root) : tab.root;
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
			<WorkbenchPanelResizeDialog view={view} />
		</div>
	);
}
