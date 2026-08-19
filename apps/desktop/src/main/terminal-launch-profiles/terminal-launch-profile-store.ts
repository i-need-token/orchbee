import type { DesktopTerminalLaunchProfileCatalog } from '@orchbee/desktop-client-protocol';

export type TerminalLaunchProfileStoreLoadResult =
	| { readonly catalog: DesktopTerminalLaunchProfileCatalog; readonly kind: 'available' }
	| { readonly kind: 'missing' }
	| { readonly kind: 'unavailable'; readonly reason: 'invalid-record' | 'read-failed' };

export interface TerminalLaunchProfileStore {
	load(): Promise<TerminalLaunchProfileStoreLoadResult>;
	replace(catalog: DesktopTerminalLaunchProfileCatalog): Promise<void>;
}
