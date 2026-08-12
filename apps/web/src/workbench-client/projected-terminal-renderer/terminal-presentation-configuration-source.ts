import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
	type TerminalPresentationConfiguration,
} from '@orchbee/terminal';

/** Stable application-owned source observed by an existing projected-terminal attachment. */
export interface TerminalPresentationConfigurationSource {
	current(): TerminalPresentationConfiguration;
	subscribe(listener: (configuration: TerminalPresentationConfiguration) => void): () => void;
}

export const DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE: TerminalPresentationConfigurationSource =
	Object.freeze({
		current: () => DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
		subscribe: () => () => undefined,
	});
