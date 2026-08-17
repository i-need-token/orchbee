import type {
	TerminalPresentationId,
	WorkbenchTerminalPresentationEventResult,
} from '@orchbee/workbench';
import type { ProjectedTerminalMetadata } from '@orchbee/terminal';
import { createContext } from 'react';

import type {
	ProjectedTerminalShortcutBindingSource,
	ProjectedTerminalShortcutPlatform,
} from '../../projected-terminal-actions';
import type { TerminalPresentationConfigurationSource } from '../../../terminal-presentation';
import type { ProjectedTerminalPlatformEffects } from '../../projected-terminal-platform-effects';
import type { TerminalProjectionClient } from '../../terminal-projection';
import type { TerminalFileDropCapability } from '../../../terminal-file-drop';
import type { TerminalCompletionReminderObserver } from '../../../terminal-feedback-settings';

export interface ProjectedTerminalSurfaceCapability {
	readonly documentElement: Pick<HTMLElement, 'dir' | 'lang'>;
	readonly preferredLocales: readonly string[];
	readonly platformEffects: ProjectedTerminalPlatformEffects;
	readonly projectionClient: Pick<TerminalProjectionClient, 'open'>;
	readonly shortcutBindingSource?: ProjectedTerminalShortcutBindingSource | undefined;
	readonly shortcutPlatform: ProjectedTerminalShortcutPlatform;
	readonly terminalFileDropCapability?: TerminalFileDropCapability | undefined;
	readonly terminalCompletionReminderObserver?: TerminalCompletionReminderObserver | undefined;
	readonly presentationConfigurationSource: TerminalPresentationConfigurationSource;
	observeTerminalMetadata(
		presentationId: TerminalPresentationId,
		metadata: Pick<ProjectedTerminalMetadata, 'closeActivity' | 'currentDirectory' | 'title'>,
	): void;
	startStoppedTerminalSession(
		presentationId: TerminalPresentationId,
	): WorkbenchTerminalPresentationEventResult;
}

export const ProjectedTerminalSurfaceCapabilityContext =
	createContext<ProjectedTerminalSurfaceCapability | null>(null);
