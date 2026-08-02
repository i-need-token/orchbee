import * as React from 'react';

import type { PanelLayoutNode } from '../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../types/panel-workspace-view';
import type { WorkbenchTerminalSurface } from '../../../../types/workbench-terminal-surface';
import { WorkbenchPanel } from './components/workbench-panel/WorkbenchPanel';
import { WorkbenchPanelSplit } from './components/workbench-panel-split/WorkbenchPanelSplit';

interface WorkbenchPanelLayoutProps {
	readonly node: PanelLayoutNode;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchPanelLayout({
	node,
	terminalSurface,
	view,
}: WorkbenchPanelLayoutProps): React.ReactNode {
	if (node.kind === 'panel') {
		return <WorkbenchPanel panel={node} terminalSurface={terminalSurface} view={view} />;
	}

	return (
		<WorkbenchPanelSplit
			renderNode={(childNode) => (
				<WorkbenchPanelLayout node={childNode} terminalSurface={terminalSurface} view={view} />
			)}
			split={node}
			view={view}
		/>
	);
}
