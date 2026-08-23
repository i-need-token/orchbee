import {
	DEFAULT_TERMINAL_INTERACTION_CONFIGURATION,
	type TerminalInteractionConfiguration,
} from '@orchbee/terminal';

export interface TerminalInteractionPreferences {
	readonly copyOnSelect: boolean;
}

export const DEFAULT_TERMINAL_INTERACTION_PREFERENCES: TerminalInteractionPreferences =
	DEFAULT_TERMINAL_INTERACTION_CONFIGURATION;

export function terminalInteractionConfigurationFromPreferences(
	preferences: TerminalInteractionPreferences,
): TerminalInteractionConfiguration {
	return Object.freeze({ copyOnSelect: preferences.copyOnSelect });
}
