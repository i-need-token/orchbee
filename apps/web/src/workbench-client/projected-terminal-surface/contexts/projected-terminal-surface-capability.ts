import type {
	WorkbenchAgentSource,
	WorkbenchTerminalPresentationEventResult,
} from '@orchbee/workbench';
import type {
	ProjectedTerminalMetadata,
	ProjectedTerminalPresentationSnapshot,
	TerminalInteractionConfigurationSource,
} from '@orchbee/terminal';
import { createContext } from 'react';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { ApplicationLocaleSource } from '../../../application-locale';
import type {
	ProjectedTerminalShortcutBindingSource,
	ProjectedTerminalShortcutPlatform,
} from '../../projected-terminal-actions';
import type { TerminalPresentationConfigurationSource } from '../../../terminal-presentation';
import type { ProjectedTerminalPlatformEffects } from '../../projected-terminal-platform-effects';
import type { TerminalProjectionClient } from '../../terminal-projection';
import type { TerminalFileDropCapability } from '../../../terminal-file-drop';
import type { TerminalCompletionReminderObserver } from '../../../terminal-feedback-settings';
import type { TerminalInputConfigurationSource } from '../../../terminal-input-configuration';
import type { TerminalTrailingBlankRowsPreferenceSource } from '../../../terminal-interaction-settings';

/** Why a terminal launch failed, as the Server recorded it. */
interface ProjectedTerminalLaunchFailure {
	/** The program that could not start, when the launch named one. */
	readonly executable: string | null;
}

export interface ProjectedTerminalSurfaceCapability {
	readonly agentObservationSource?: WorkbenchAgentSource | undefined;
	readonly applicationLocaleSource?: ApplicationLocaleSource | undefined;
	readonly documentElement: Pick<HTMLElement, 'dir' | 'lang'>;
	readonly preferredLocales: readonly string[];
	readonly platformEffects: ProjectedTerminalPlatformEffects;
	readonly projectionClient: Pick<TerminalProjectionClient, 'open'>;
	/**
	 * The recorded failure of a terminal whose creation failed, or null when nothing launched it. A
	 * Client without this record reports every failed creation without its reason.
	 */
	readonly readTerminalLaunchFailure?:
		| ((presentationId: TerminalPresentationId) => ProjectedTerminalLaunchFailure | null)
		| undefined;
	readonly presentationSnapshots?:
		| {
				get(
					presentationId: TerminalPresentationId,
				): ProjectedTerminalPresentationSnapshot | undefined;
				set(
					presentationId: TerminalPresentationId,
					snapshot: ProjectedTerminalPresentationSnapshot,
				): boolean;
		  }
		| undefined;
	readonly shortcutBindingSource?: ProjectedTerminalShortcutBindingSource | undefined;
	readonly shortcutPlatform: ProjectedTerminalShortcutPlatform;
	readonly terminalFileDropCapability?: TerminalFileDropCapability | undefined;
	readonly terminalCompletionReminderObserver?: TerminalCompletionReminderObserver | undefined;
	readonly presentationConfigurationSource: TerminalPresentationConfigurationSource;
	readonly inputConfigurationSource?: TerminalInputConfigurationSource | undefined;
	readonly interactionConfigurationSource?: TerminalInteractionConfigurationSource | undefined;
	readonly trailingBlankRowsPreferenceSource?:
		| TerminalTrailingBlankRowsPreferenceSource
		| undefined;
	observeTerminalMetadata(
		presentationId: TerminalPresentationId,
		metadata: Pick<
			ProjectedTerminalMetadata,
			'closeActivity' | 'currentDirectory' | 'revision' | 'title'
		>,
	): void;
	startStoppedTerminalSession(
		presentationId: TerminalPresentationId,
	): WorkbenchTerminalPresentationEventResult;
}

export const ProjectedTerminalSurfaceCapabilityContext =
	createContext<ProjectedTerminalSurfaceCapability | null>(null);
