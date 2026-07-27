import * as React from 'react';

import { getDisplayedTerminal, type PanelLeaf } from '../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../types/panel-workspace-view';
import { WorkbenchChildTerminalStrip } from './components/workbench-child-terminal-strip/WorkbenchChildTerminalStrip';
import { WorkbenchPanelDropPlaceholder } from './components/workbench-panel-drop-placeholder/WorkbenchPanelDropPlaceholder';
import { WorkbenchPanelFocusScrim } from './components/workbench-panel-focus-scrim/WorkbenchPanelFocusScrim';
import { WorkbenchPanelHeader } from './components/workbench-panel-header/WorkbenchPanelHeader';
import { WorkbenchTerminalViewport } from './components/workbench-terminal-viewport/WorkbenchTerminalViewport';
import styles from './WorkbenchPanel.module.css';

interface WorkbenchPanelProps {
	readonly panel: PanelLeaf;
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchPanel({ panel, view }: WorkbenchPanelProps): React.ReactNode {
	const displayedTerminal = getDisplayedTerminal(panel);
	const showHeader = view.panelCount > 1 || panel.activeChildTerminalId !== null;
	const isDropPlaceholder = view.panelDrag.draggingPanelId === panel.id;
	const isInactive =
		view.panelCount > 1 && view.activeTab !== null && view.activeTab.focusedPanelId !== panel.id;

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
			onPointerDown={() => view.focusPanel(panel.id, 'pointer')}
		>
			{showHeader && <WorkbenchPanelHeader panel={panel} view={view} />}
			<WorkbenchTerminalViewport
				panelId={panel.id}
				terminal={displayedTerminal}
				terminalRole={panel.activeChildTerminalId === null ? 'primary' : 'child'}
				view={view}
			/>
			{panel.childTerminals.length > 0 && <WorkbenchChildTerminalStrip panel={panel} view={view} />}
			{isInactive && <WorkbenchPanelFocusScrim />}
		</section>
	);
}
