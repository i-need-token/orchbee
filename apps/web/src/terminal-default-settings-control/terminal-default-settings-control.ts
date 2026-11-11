import type { TerminalConfiguration } from '@orchbee/terminal-protocol';

/**
 * The terminal defaults Settings shows. Scrollback is saved in the settings file, so this control
 * only reads; it is unavailable until the first settings snapshot arrives.
 */
export interface TerminalDefaultSettingsClientSnapshot {
	readonly configuration: TerminalConfiguration;
	readonly failureCode: 'control-unavailable' | null;
}

export interface TerminalDefaultSettingsControl {
	getSnapshot(): TerminalDefaultSettingsClientSnapshot;
	subscribe(listener: () => void): () => void;
}
