import {
	UserLaunchProfileCatalogSchema,
	type SettingsRefusal,
	type UserLaunchProfile,
	type UserLaunchProfileId,
} from '@orchbee/workbench-client-protocol';
import { z } from 'zod';

/** @internal Fixture parser for the Web terminal settings Stories and tests; no production boundary parses it. */
export const TerminalLaunchProfileSnapshotSchema = z.strictObject({
	/** The settings revision that last changed the catalog; a draft begun before it is stale. */
	catalogRevision: z.string().optional(),
	capabilities: z.strictObject({
		loginShell: z.boolean(),
	}),
	catalog: UserLaunchProfileCatalogSchema,
	/** Whether the settings file sets the catalog or its default; built-in means Restore has nothing to do. */
	source: z.enum(['built-in', 'stored']),
});

const TerminalLaunchProfileFailureCodeSchema = z.enum([
	'busy',
	'catalog-full',
	'control-unavailable',
	'default-profile-delete',
	'identity-allocation-failed',
	'invalid-profile',
	'last-profile-delete',
	'profile-not-found',
	'unsupported-platform',
	'write-failed',
	'source-changed',
	'outcome-unknown',
	'refused',
]);

const TerminalLaunchProfileResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('completed'),
		snapshot: TerminalLaunchProfileSnapshotSchema,
	}),
	z.strictObject({
		code: TerminalLaunchProfileFailureCodeSchema,
		kind: z.literal('failed'),
		snapshot: TerminalLaunchProfileSnapshotSchema,
	}),
]);

export type TerminalLaunchProfileFailureCode = z.infer<
	typeof TerminalLaunchProfileFailureCodeSchema
>;
export type TerminalLaunchProfileSnapshot = z.infer<typeof TerminalLaunchProfileSnapshotSchema>;

export interface TerminalLaunchProfileClientSnapshot extends TerminalLaunchProfileSnapshot {
	readonly failureCode: TerminalLaunchProfileFailureCode | null;
	readonly pending: boolean;
	/** The settings file refuses every edit while it does not parse or cannot be read. */
	readonly readOnly: boolean;
	/** Why the settings file refused the last change, when `failureCode` is `refused`. */
	readonly refusal: SettingsRefusal | null;
}

export type TerminalLaunchProfileClientOperationResult =
	| z.infer<typeof TerminalLaunchProfileResultSchema>
	| { readonly kind: 'unavailable' };

export interface TerminalLaunchProfileControl {
	createProfile(
		name: string,
		sourceProfileId: UserLaunchProfileId,
	): Promise<TerminalLaunchProfileClientOperationResult>;
	deleteProfile(
		profileId: UserLaunchProfileId,
	): Promise<TerminalLaunchProfileClientOperationResult>;
	getSnapshot(): TerminalLaunchProfileClientSnapshot;
	reset(): Promise<TerminalLaunchProfileClientOperationResult>;
	selectDefault(
		profileId: UserLaunchProfileId,
	): Promise<TerminalLaunchProfileClientOperationResult>;
	subscribe(listener: () => void): () => void;
	updateProfile(
		profile: UserLaunchProfile,
		catalogRevision?: string,
	): Promise<TerminalLaunchProfileClientOperationResult>;
}
