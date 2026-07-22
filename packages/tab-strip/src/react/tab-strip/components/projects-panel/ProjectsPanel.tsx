import type { ReactNode } from 'react';
import type { ProjectsPanelController } from '../../../../projects';

import { useProjectsPanel } from './hooks/useProjectsPanel';

export type ProjectsPanelCloseReason = 'button' | 'escape' | 'outside-pointer' | 'focus-lost';

export interface ProjectsPanelProps {
	controller: ProjectsPanelController;
	open: boolean;
	onClose: (reason: ProjectsPanelCloseReason) => void;
	targetWidth?: number;
	elevated?: boolean;
	title?: string;
	className?: string;
}

export function ProjectsPanel(props: ProjectsPanelProps): ReactNode {
	return useProjectsPanel(props);
}
