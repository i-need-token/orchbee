import * as React from 'react';

import type { PanelLayoutNode } from '../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../types/panel-workspace-view';
import { WorkbenchPanel } from './components/workbench-panel/WorkbenchPanel';
import { WorkbenchPanelSplit } from './components/workbench-panel-split/WorkbenchPanelSplit';

interface WorkbenchPanelLayoutProps {
	readonly node: PanelLayoutNode;
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchPanelLayout({ node, view }: WorkbenchPanelLayoutProps): React.ReactNode {
	if (node.kind === 'panel') {
		return <WorkbenchPanel panel={node} view={view} />;
	}

	return (
		<WorkbenchPanelSplit
			renderNode={(childNode) => <WorkbenchPanelLayout node={childNode} view={view} />}
			split={node}
			view={view}
		/>
	);
}
