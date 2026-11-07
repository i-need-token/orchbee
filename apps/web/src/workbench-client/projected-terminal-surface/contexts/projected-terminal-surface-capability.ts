import type { WorkbenchAgentSource } from '../../../workbench';
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
import type { TerminalCompletionReminderObserver } from '../../../terminal-feedback-settings';
import type { TerminalInputConfigurationSource } from '../../../terminal-input-configuration';
import type { TerminalTrailingBlankRowsPreferenceSource } from '../../../terminal-interaction-settings';

/**
 * Why a terminal shows no running terminal, as the Server recorded it: its launch failed, or a
 * restore held it because its saved directory was unavailable.
 */
type ProjectedTerminalLaunchFailure =
	| {
			/** The program that could not start, when the launch named one. */
			readonly executable: string | null;
			readonly kind: 'failed';
	  }
	| {
			/** A retained Agent conversation waits to resume in that directory. */
			readonly agentResume: boolean;
			readonly directory: string;
			readonly kind: 'directory-unavailable';
	  };

export interface ProjectedTerminalSurfaceCapability {
	readonly agentObservationSource?: WorkbenchAgentSource | undefined;
	readonly applicationLocaleSource?: ApplicationLocaleSource | undefined;
	readonly documentElement: Pick<HTMLElement, 'dir' | 'lang'>;
	readonly preferredLocales: readonly string[];
	readonly platformEffects: ProjectedTerminalPlatformEffects;
	readonly projectionClient: Pick<TerminalProjectionClient, 'open'>;
	/**
	 * The recorded failure of a terminal whose creation failed or whose restore was held, or null when
	 * nothing launched it. A Client without this record reports every failed creation without its
	 * reason.
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
	readonly terminalCompletionReminderObserver?: TerminalCompletionReminderObserver | undefined;
	readonly presentationConfigurationSource: TerminalPresentationConfigurationSource;
	readonly inputConfigurationSource?: TerminalInputConfigurationSource | undefined;
	readonly interactionConfigurationSource?: TerminalInteractionConfigurationSource | undefined;
	readonly trailingBlankRowsPreferenceSource?:
		| TerminalTrailingBlankRowsPreferenceSource
		| undefined;
	observeTerminalMetadata(
		presentationId: TerminalPresentationId,
		metadata: Pick<ProjectedTerminalMetadata, 'currentDirectory' | 'revision' | 'title'>,
	): void;
}

export const ProjectedTerminalSurfaceCapabilityContext =
	createContext<ProjectedTerminalSurfaceCapability | null>(null);
