import {
	TerminalOptionAsAltMode,
	type TerminalInputConfiguration,
	type TerminalOptionAsAltMode as TerminalOptionAsAltModeValue,
} from '@orchbee/terminal';

export interface MacosTerminalInputPreferences {
	readonly optionAsAlt: TerminalOptionAsAltModeValue;
}

export const DEFAULT_MACOS_TERMINAL_INPUT_PREFERENCES: MacosTerminalInputPreferences =
	Object.freeze({ optionAsAlt: TerminalOptionAsAltMode.off });

export function terminalInputConfigurationFromMacosPreferences(
	preferences: MacosTerminalInputPreferences,
): TerminalInputConfiguration {
	return Object.freeze({ optionAsAlt: preferences.optionAsAlt });
}
