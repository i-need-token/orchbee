export const TerminalOptionAsAltMode = {
	both: 'both',
	left: 'left',
	off: 'off',
	right: 'right',
} as const;

export type TerminalOptionAsAltMode =
	(typeof TerminalOptionAsAltMode)[keyof typeof TerminalOptionAsAltMode];

export interface TerminalInputConfiguration {
	readonly optionAsAlt: TerminalOptionAsAltMode;
}

export const DEFAULT_TERMINAL_INPUT_CONFIGURATION: TerminalInputConfiguration = Object.freeze({
	optionAsAlt: TerminalOptionAsAltMode.off,
});

export function isTerminalOptionAsAltMode(value: unknown): value is TerminalOptionAsAltMode {
	return value === 'both' || value === 'left' || value === 'off' || value === 'right';
}
