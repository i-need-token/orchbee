import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import * as React from 'react';

import type { WorkbenchSharedPanelTerminalCreate } from '../../actions/createSharedPanelTerminal';
import type { PanelWorkspaceStore } from './model/PanelWorkspaceStore';
import type { WorkbenchAgentStatusView } from '../../utils/formatWorkbenchAgentStatus';
import type { WorkbenchAuthority } from '../../types/workbench-authority';
import type { WorkbenchTerminalSurface } from '../../types/workbench-terminal-surface';
import { WorkbenchPanelCloseDialog } from './components/workbench-panel-close-dialog/WorkbenchPanelCloseDialog';
import { WorkbenchPanelDragGhost } from './components/workbench-panel-drag-ghost/WorkbenchPanelDragGhost';
import { WorkbenchPanelLayout } from './components/workbench-panel-layout/WorkbenchPanelLayout';
import { WorkbenchPanelResizeDialog } from './components/workbench-panel-resize-dialog/WorkbenchPanelResizeDialog';
import { useWorkbenchPanelWorkspace } from './hooks/useWorkbenchPanelWorkspace';
import { useTerminalHandoffKeystrokes } from './hooks/useTerminalHandoffKeystrokes';
import { useTerminalTabPresentationHandoff } from './hooks/useTerminalTabPresentationHandoff';
import { projectTerminalTabPresentationLayers } from './model/projectTerminalTabPresentationLayers';
import { projectWorkbenchPanelWorkspaceTabView } from './model/projectWorkbenchPanelWorkspaceTabView';
import type { TerminalTabPresentationLayer } from './types/terminal-tab-presentation-layer';
import type { WorkbenchTerminalFocusRequest } from '../../model/resolveWorkbenchFocusTarget';
import styles from './WorkbenchPanelWorkspace.module.css';

interface WorkbenchPanelWorkspaceProps {
	readonly onTerminalEdit?: ((presentationId: string) => void) | undefined;
	readonly actionRuntime: ActionRuntime;
	readonly authority: WorkbenchAuthority;
	readonly createPanelTerminal: WorkbenchSharedPanelTerminalCreate;
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly parentActionScope: ActionScope;
	readonly presentationAgentStatuses: ReadonlyMap<string, WorkbenchAgentStatusView>;
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	readonly terminalAutoFocusAllowed: boolean;
	readonly terminalContextId: string;
	readonly terminalFocusRequest: WorkbenchTerminalFocusRequest | null;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
}

export function WorkbenchPanelWorkspace({
	onTerminalEdit,
	actionRuntime,
	authority,
	createPanelTerminal,
	panelWorkspaceStore,
	parentActionScope,
	presentationAgentStatuses,
	shortcutLabelForAction,
	terminalAutoFocusAllowed,
	terminalContextId,
	terminalFocusRequest,
	terminalSurface,
}: WorkbenchPanelWorkspaceProps): React.ReactNode {
	const view = useWorkbenchPanelWorkspace({
		onTerminalEdit,
		actionRuntime,
		authority,
		createPanelTerminal,
		panelWorkspaceStore,
		parentActionScope,
		presentationAgentStatuses,
		shortcutLabelForAction,
		terminalContextId,
	});
	const handoff = useTerminalTabPresentationHandoff({
		activeTab: view.activeTab,
		presentationScopeId: terminalContextId,
		tabs: view.state.tabs,
		terminalSurfaceAvailable: terminalSurface !== undefined,
	});
	useTerminalHandoffKeystrokes(handoff.state.kind === 'settled');
	const presentationLayers = projectTerminalTabPresentationLayers(
		view.activeTab,
		handoff.state,
		handoff.displayedTab,
	);
	const { releasePanelDragHold } = view;
	const heldRevision = view.panelDrag.ghost === null ? view.panelDrag.sourceRevision : null;
	const holdShown = presentationLayers.some((layer) => layer.revision === heldRevision);
	React.useLayoutEffect(() => {
		if (heldRevision !== null && !holdShown) releasePanelDragHold(heldRevision);
	}, [heldRevision, holdShown, releasePanelDragHold]);

	return (
		<div
			aria-label={view.panelVisualSelectionLabel}
			className={styles['root']}
			data-dragging={view.panelDrag.ghost !== null || undefined}
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
			{presentationLayers.map(({ epoch, mode, revision, tab }) => {
				const layer: TerminalTabPresentationLayer = {
					epoch,
					mode,
					reportPresentationReady: (presentationId, readiness) => {
						handoff.reportPresentationReady(epoch, tab.tabId, revision, presentationId, readiness);
					},
				};
				const layerView = projectWorkbenchPanelWorkspaceTabView(view, tab, revision);
				const node = layerView.panelDrag.previewRoot ?? tab.root;
				return (
					<div
						aria-hidden={mode === 'active' ? undefined : true}
						className={styles['presentationLayer']}
						data-presentation-mode={mode}
						data-terminal-tab-id={tab.tabId}
						inert={mode === 'active' ? undefined : true}
						key={revision}
					>
						<WorkbenchPanelLayout
							layer={layer}
							node={node}
							terminalAutoFocusAllowed={terminalAutoFocusAllowed}
							terminalFocusRequest={terminalFocusRequest}
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
