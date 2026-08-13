import type {
	TerminalPresentationId,
	WorkbenchTerminalPresentationEventResult,
} from '@orchbee/workbench';
import type { ProjectedTerminalMetadata } from '@orchbee/terminal';
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
	observeTerminalMetadata(
		presentationId: TerminalPresentationId,
		metadata: Pick<ProjectedTerminalMetadata, 'currentDirectory' | 'title'>,
	): void;
	startStoppedTerminalSession(
		presentationId: TerminalPresentationId,
	): WorkbenchTerminalPresentationEventResult;
}

export const ProjectedTerminalSurfaceCapabilityContext =
	createContext<ProjectedTerminalSurfaceCapability | null>(null);
