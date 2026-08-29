import type { AgentLocationPresentation } from '@orchbee/agent';
import * as React from 'react';

import type { PanelLayoutNode } from '../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../types/panel-workspace-view';
import type { WorkbenchTerminalSurface } from '../../../../types/workbench-terminal-surface';
import type { TerminalTabPresentationLayer } from '../../types/terminal-tab-presentation-layer';
import type { WorkbenchTerminalFocusRequest } from '../../../../model/resolveWorkbenchFocusTarget';
import { WorkbenchPanel } from './components/workbench-panel/WorkbenchPanel';
import { WorkbenchPanelSplit } from './components/workbench-panel-split/WorkbenchPanelSplit';

interface WorkbenchPanelLayoutProps {
	readonly agentLocations?: ReadonlyMap<string, AgentLocationPresentation> | undefined;
	readonly layer: TerminalTabPresentationLayer;
	readonly layoutSettled?: boolean | undefined;
	readonly node: PanelLayoutNode;
	readonly terminalAutoFocusAllowed: boolean;
	readonly terminalFocusRequest: WorkbenchTerminalFocusRequest | null;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchPanelLayout({
	agentLocations,
	layer,
	layoutSettled = true,
	node,
	terminalAutoFocusAllowed,
	terminalFocusRequest,
	terminalSurface,
	view,
}: WorkbenchPanelLayoutProps): React.ReactNode {
	if (node.kind === 'panel') {
		return (
			<WorkbenchPanel
				agentLocations={agentLocations}
				layer={layer}
				layoutSettled={layoutSettled}
				panel={node}
				terminalAutoFocusAllowed={terminalAutoFocusAllowed}
				terminalFocusRequest={terminalFocusRequest}
				terminalSurface={terminalSurface}
				view={view}
			/>
		);
	}

	return (
		<WorkbenchPanelSplit
			renderNode={(childNode, childLayoutSettled) => (
				<WorkbenchPanelLayout
					agentLocations={agentLocations}
					layer={layer}
					layoutSettled={layoutSettled && childLayoutSettled}
					node={childNode}
					terminalAutoFocusAllowed={terminalAutoFocusAllowed}
					terminalFocusRequest={terminalFocusRequest}
					terminalSurface={terminalSurface}
					view={view}
				/>
			)}
			split={node}
			view={view}
		/>
	);
}
