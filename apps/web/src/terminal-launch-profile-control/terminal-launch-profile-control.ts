import {
	UserLaunchProfileCatalogSchema,
	UserPreferencesSourceSchema,
	type UserLaunchProfile,
	type UserLaunchProfileId,
} from '@orchbee/workbench-client-protocol';
import { z } from 'zod';

const TerminalLaunchProfilePersistenceIssueSchema = z.enum([
	'invalid-record',
	'read-failed',
	'write-failed',
]);

/** @internal Fixture parser for the Web terminal settings Stories and tests; no production boundary parses it. */
export const TerminalLaunchProfileSnapshotSchema = z.strictObject({
	preferencesSource: UserPreferencesSourceSchema.optional(),
	capabilities: z.strictObject({
		loginShell: z.boolean(),
	}),
	catalog: UserLaunchProfileCatalogSchema,
	persistenceIssue: TerminalLaunchProfilePersistenceIssueSchema.nullable(),
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
		preferencesSource?: string,
	): Promise<TerminalLaunchProfileClientOperationResult>;
}
