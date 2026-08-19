import type {
	DesktopTerminalLaunchProfile,
	DesktopTerminalLaunchProfileFailureCode,
	DesktopTerminalLaunchProfileId,
	DesktopTerminalLaunchProfileMainEvent,
	DesktopTerminalLaunchProfileSnapshot,
} from '@orchbee/desktop-client-protocol';

export interface DesktopTerminalLaunchProfileClientSnapshot extends DesktopTerminalLaunchProfileSnapshot {
	readonly failureCode: DesktopTerminalLaunchProfileFailureCode | null;
	readonly pending: boolean;
}

export type DesktopTerminalLaunchProfileClientInitialization =
	| { readonly kind: 'ready' }
	| { readonly kind: 'unavailable' };

export type DesktopTerminalLaunchProfileClientOperationResult =
	| DesktopTerminalLaunchProfileMainEvent['result']
	| { readonly kind: 'unavailable' };

export interface DesktopTerminalLaunchProfileControl {
	createProfile(
		name: string,
		sourceProfileId: DesktopTerminalLaunchProfileId,
	): Promise<DesktopTerminalLaunchProfileClientOperationResult>;
	deleteProfile(
		profileId: DesktopTerminalLaunchProfileId,
	): Promise<DesktopTerminalLaunchProfileClientOperationResult>;
	getSnapshot(): DesktopTerminalLaunchProfileClientSnapshot;
	reset(): Promise<DesktopTerminalLaunchProfileClientOperationResult>;
	selectDefault(
		profileId: DesktopTerminalLaunchProfileId,
	): Promise<DesktopTerminalLaunchProfileClientOperationResult>;
	subscribe(listener: () => void): () => void;
	updateProfile(
		profile: DesktopTerminalLaunchProfile,
	): Promise<DesktopTerminalLaunchProfileClientOperationResult>;
}
