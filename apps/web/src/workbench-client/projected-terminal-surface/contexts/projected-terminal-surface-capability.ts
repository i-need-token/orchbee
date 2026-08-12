import type { TerminalPresentationId } from '@orchbee/workbench';
import { createContext } from 'react';

import type { ProjectedTerminalShortcutPlatform } from '../../projected-terminal-actions';
import type { TerminalPresentationConfigurationSource } from '../../projected-terminal-renderer';
import type { ProjectedTerminalPlatformEffects } from '../../projected-terminal-platform-effects';
import type { TerminalProjectionClient } from '../../terminal-projection';

export interface ProjectedTerminalSurfaceCapability {
	readonly documentElement: Pick<HTMLElement, 'dir' | 'lang'>;
	readonly preferredLocales: readonly string[];
	readonly platformEffects: ProjectedTerminalPlatformEffects;
	readonly projectionClient: Pick<TerminalProjectionClient, 'open'>;
	readonly shortcutPlatform: ProjectedTerminalShortcutPlatform;
	readonly presentationConfigurationSource: TerminalPresentationConfigurationSource;
	startStoppedTerminalSession(presentationId: TerminalPresentationId): void;
}

export const ProjectedTerminalSurfaceCapabilityContext =
	createContext<ProjectedTerminalSurfaceCapability | null>(null);
