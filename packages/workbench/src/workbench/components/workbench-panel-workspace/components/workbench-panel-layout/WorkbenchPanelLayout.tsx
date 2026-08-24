import * as React from 'react';

import type { PanelLayoutNode } from '../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../types/panel-workspace-view';
import type { WorkbenchTerminalSurface } from '../../../../types/workbench-terminal-surface';
import type { TerminalTabPresentationLayer } from '../../types/terminal-tab-presentation-layer';
import { WorkbenchPanel } from './components/workbench-panel/WorkbenchPanel';
import { WorkbenchPanelSplit } from './components/workbench-panel-split/WorkbenchPanelSplit';

interface WorkbenchPanelLayoutProps {
	readonly layer: TerminalTabPresentationLayer;
	readonly node: PanelLayoutNode;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchPanelLayout({
	layer,
	node,
	terminalSurface,
	view,
}: WorkbenchPanelLayoutProps): React.ReactNode {
	if (node.kind === 'panel') {
		return (
			<WorkbenchPanel layer={layer} panel={node} terminalSurface={terminalSurface} view={view} />
		);
	}

	return (
		<WorkbenchPanelSplit
			renderNode={(childNode) => (
				<WorkbenchPanelLayout
					layer={layer}
					node={childNode}
					terminalSurface={terminalSurface}
					view={view}
				/>
			)}
			split={node}
			view={view}
		/>
	);
}
