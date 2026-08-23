import type { TerminalConfiguration } from '@orchbee/terminal-protocol';

export type TerminalDefaultSettingsStoreLoadResult =
	| { readonly configuration: TerminalConfiguration; readonly kind: 'available' }
	| { readonly kind: 'missing' }
	| { readonly kind: 'unavailable'; readonly reason: 'invalid-record' | 'read-failed' };

export interface TerminalDefaultSettingsStore {
	load(): Promise<TerminalDefaultSettingsStoreLoadResult>;
	replace(configuration: TerminalConfiguration): Promise<void>;
}
