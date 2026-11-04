import * as React from 'react';

import { getDisplayedTerminal, type PanelLeaf } from '../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../types/panel-workspace-view';
import type { TerminalTabPresentationLayer } from '../../../../types/terminal-tab-presentation-layer';
import type { WorkbenchTerminalSurface } from '../../../../../../types/workbench-terminal-surface';
import type { WorkbenchTerminalFocusRequest } from '../../../../../../model/resolveWorkbenchFocusTarget';
import { WorkbenchChildTerminalStrip } from './components/workbench-child-terminal-strip/WorkbenchChildTerminalStrip';
import { WorkbenchPanelDropPlaceholder } from './components/workbench-panel-drop-placeholder/WorkbenchPanelDropPlaceholder';
import { WorkbenchPanelFocusScrim } from './components/workbench-panel-focus-scrim/WorkbenchPanelFocusScrim';
import { WorkbenchPanelHeader } from './components/workbench-panel-header/WorkbenchPanelHeader';
import { WorkbenchPanelBroadcastTarget } from './components/workbench-panel-broadcast-target/WorkbenchPanelBroadcastTarget';
import { WorkbenchPanelVisualSelectionTarget } from './components/workbench-panel-visual-selection-target/WorkbenchPanelVisualSelectionTarget';
import { WorkbenchTerminalViewport } from './components/workbench-terminal-viewport/WorkbenchTerminalViewport';
import styles from './WorkbenchPanel.module.css';

interface WorkbenchPanelProps {
	readonly layer: TerminalTabPresentationLayer;
	readonly layoutSettled: boolean;
	readonly panel: PanelLeaf;
	readonly terminalAutoFocusAllowed: boolean;
	readonly terminalFocusRequest: WorkbenchTerminalFocusRequest | null;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchPanel({
	layer,
	layoutSettled,
	panel,
	terminalAutoFocusAllowed,
	terminalFocusRequest,
	terminalSurface,
	view,
}: WorkbenchPanelProps): React.ReactNode {
	const displayedTerminal = getDisplayedTerminal(panel);
	const showHeader = view.panelCount > 1 || panel.activeChildPresentationId !== null;
	const isDropPlaceholder = view.panelDrag.draggingPanelId === panel.id;
	const isInactive =
		view.panelCount > 1 && view.activeTab !== null && view.activeTab.focusedPanelId !== panel.id;
	const visualSelectionTarget = view.panelVisualSelection?.targets.find(
		(target) => target.panelId === panel.id,
	);
	const isBroadcastTarget =
		view.broadcastInput.kind === 'active' &&
		view.broadcastInput.snapshot.targets.some((target) => target.panelId === panel.id);

	if (isDropPlaceholder) {
		return (
			<section
				className={styles['root']}
				data-drop-placeholder='true'
				data-panel-id={panel.id}
				data-slot='workbench-panel'
			>
				<WorkbenchPanelDropPlaceholder />
			</section>
		);
	}

	return (
		<section
			className={styles['root']}
			data-focused={view.activeTab?.focusedPanelId === panel.id || undefined}
			data-inactive={isInactive || undefined}
			data-panel-id={panel.id}
			data-slot='workbench-panel'
			onFocusCapture={() => view.focusPanel(panel.id, 'programmatic')}
			onPointerDown={(event) => {
				if (event.button === 0) view.focusPanel(panel.id, 'pointer');
			}}
		>
			{showHeader && <WorkbenchPanelHeader panel={panel} view={view} />}
			<WorkbenchTerminalViewport
				layer={layer}
				layoutSettled={layoutSettled}
				panelId={panel.id}
				terminal={displayedTerminal}
				terminalRole={panel.activeChildPresentationId === null ? 'primary' : 'child'}
				terminalAutoFocusAllowed={terminalAutoFocusAllowed}
				terminalFocusRequest={terminalFocusRequest}
				terminalSurface={terminalSurface}
				view={view}
			/>
			{panel.childPresentations.length > 0 && (
				<WorkbenchChildTerminalStrip panel={panel} view={view} />
			)}
			{isInactive && <WorkbenchPanelFocusScrim />}
			{view.panelVisualSelection !== null && visualSelectionTarget !== undefined && (
				<WorkbenchPanelVisualSelectionTarget
					label={visualSelectionTarget.label}
					messages={view.messages}
					mode={view.panelVisualSelection.mode}
					panelId={panel.id}
					prefix={view.panelVisualSelection.prefix}
				/>
			)}
			{isBroadcastTarget && <WorkbenchPanelBroadcastTarget />}
		</section>
	);
}
