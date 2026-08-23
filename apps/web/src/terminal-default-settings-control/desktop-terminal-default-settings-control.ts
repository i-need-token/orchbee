import type {
	DesktopTerminalDefaultSettingsFailureCode,
	DesktopTerminalDefaultSettingsMainEvent,
	DesktopTerminalDefaultSettingsSnapshot,
} from '@orchbee/desktop-client-protocol';
import type { TerminalConfiguration } from '@orchbee/terminal-protocol';

export interface DesktopTerminalDefaultSettingsClientSnapshot extends DesktopTerminalDefaultSettingsSnapshot {
	readonly failureCode: DesktopTerminalDefaultSettingsFailureCode | null;
	readonly pending: boolean;
}

export type DesktopTerminalDefaultSettingsClientInitialization =
	| { readonly kind: 'ready' }
	| { readonly kind: 'unavailable' };

export type DesktopTerminalDefaultSettingsClientOperationResult =
	| DesktopTerminalDefaultSettingsMainEvent['result']
	| { readonly kind: 'unavailable' };

export interface DesktopTerminalDefaultSettingsControl {
	getSnapshot(): DesktopTerminalDefaultSettingsClientSnapshot;
	subscribe(listener: () => void): () => void;
	update(
		configuration: TerminalConfiguration,
	): Promise<DesktopTerminalDefaultSettingsClientOperationResult>;
}
