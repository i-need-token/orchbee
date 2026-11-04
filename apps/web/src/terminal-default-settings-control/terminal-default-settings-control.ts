import type { TerminalConfiguration } from '@orchbee/terminal-protocol';

/**
 * The terminal defaults Settings shows. Scrollback saves through the Session's shared preferences,
 * so this control only reads; it is unavailable until the first preference snapshot arrives.
 */
export interface TerminalDefaultSettingsClientSnapshot {
	readonly configuration: TerminalConfiguration;
	readonly failureCode: 'control-unavailable' | null;
}

export interface TerminalDefaultSettingsControl {
	getSnapshot(): TerminalDefaultSettingsClientSnapshot;
	subscribe(listener: () => void): () => void;
}
