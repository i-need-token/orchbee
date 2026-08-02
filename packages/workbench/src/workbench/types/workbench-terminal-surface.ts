import type { ComponentType } from 'react';

import type { TerminalPresentation } from '../components/workbench-panel-workspace/model/panel-workspace-state';

export type WorkbenchTerminalPresentation = TerminalPresentation;

export interface WorkbenchTerminalSurfaceProps {
	readonly presentation: WorkbenchTerminalPresentation;
}

export type WorkbenchTerminalSurface = ComponentType<WorkbenchTerminalSurfaceProps>;
