import * as React from 'react';

import type { PanelLayoutNode } from '../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../types/panel-workspace-view';
import type { WorkbenchTerminalSurface } from '../../../../types/workbench-terminal-surface';
import type { TerminalTabPresentationLayer } from '../../types/terminal-tab-presentation-layer';
import { WorkbenchPanel } from './components/workbench-panel/WorkbenchPanel';
import { WorkbenchPanelSplit } from './components/workbench-panel-split/WorkbenchPanelSplit';

interface WorkbenchPanelLayoutProps {
	readonly layer: TerminalTabPresentationLayer;
	readonly layoutSettled?: boolean | undefined;
	readonly node: PanelLayoutNode;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchPanelLayout({
	layer,
	layoutSettled = true,
	node,
	terminalSurface,
	view,
}: WorkbenchPanelLayoutProps): React.ReactNode {
	if (node.kind === 'panel') {
		return (
			<WorkbenchPanel
				layer={layer}
				layoutSettled={layoutSettled}
				panel={node}
				terminalSurface={terminalSurface}
				view={view}
			/>
		);
	}

	return (
		<WorkbenchPanelSplit
			renderNode={(childNode, childLayoutSettled) => (
				<WorkbenchPanelLayout
					layer={layer}
					layoutSettled={layoutSettled && childLayoutSettled}
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
