import {
	DEFAULT_TERMINAL_INPUT_CONFIGURATION,
	type TerminalInputConfiguration,
} from '@orchbee/terminal';

/** Stable application-owned input policy observed by existing projected terminals. */
export interface TerminalInputConfigurationSource {
	current(): TerminalInputConfiguration;
	subscribe(listener: (configuration: TerminalInputConfiguration) => void): () => void;
}

export const DEFAULT_TERMINAL_INPUT_CONFIGURATION_SOURCE: TerminalInputConfigurationSource =
	Object.freeze({
		current: () => DEFAULT_TERMINAL_INPUT_CONFIGURATION,
		subscribe: () => () => undefined,
	});
