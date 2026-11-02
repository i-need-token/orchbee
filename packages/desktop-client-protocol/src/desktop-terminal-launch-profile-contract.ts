import { z } from 'zod';
import {
	UserPreferencesSourceSchema,
	BUILT_IN_USER_LAUNCH_PROFILE_ID as DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
	USER_LAUNCH_PROFILE_LIMITS as DESKTOP_TERMINAL_LAUNCH_PROFILE_LIMITS,
	UserLaunchProfileCatalogSchema as DesktopTerminalLaunchProfileCatalogSchema,
	UserLaunchProfileIdSchema as DesktopTerminalLaunchProfileIdSchema,
	UserLaunchProfileNameSchema as DesktopTerminalLaunchProfileNameSchema,
	UserLaunchProfileSchema as DesktopTerminalLaunchProfileSchema,
} from '@orchbee/workbench-client-protocol';
export {
	DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
	DESKTOP_TERMINAL_LAUNCH_PROFILE_LIMITS,
	DesktopTerminalLaunchProfileCatalogSchema,
	DesktopTerminalLaunchProfileNameSchema,
	DesktopTerminalLaunchProfileSchema,
};
/** @internal Fixture parser for the Web terminal settings Stories and tests; no production boundary parses it. */
export { DesktopTerminalLaunchProfileIdSchema };

const DesktopTerminalLaunchProfilePersistenceIssueSchema = z.enum([
	'invalid-record',
	'read-failed',
	'write-failed',
]);

/** @internal Fixture parser for the Web terminal settings Stories and tests; no production boundary parses it. */
export const DesktopTerminalLaunchProfileSnapshotSchema = z.strictObject({
	preferencesSource: UserPreferencesSourceSchema.optional(),
	capabilities: z.strictObject({
		loginShell: z.boolean(),
	}),
	catalog: DesktopTerminalLaunchProfileCatalogSchema,
	persistenceIssue: DesktopTerminalLaunchProfilePersistenceIssueSchema.nullable(),
	source: z.enum(['built-in', 'stored']),
});

const DesktopTerminalLaunchProfileRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopTerminalLaunchProfileRequestId'>();

const DesktopTerminalLaunchProfileFailureCodeSchema = z.enum([
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

const desktopTerminalLaunchProfileResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('completed'),
		snapshot: DesktopTerminalLaunchProfileSnapshotSchema,
	}),
	z.strictObject({
		code: DesktopTerminalLaunchProfileFailureCodeSchema,
		kind: z.literal('failed'),
		snapshot: DesktopTerminalLaunchProfileSnapshotSchema,
	}),
]);

const DesktopTerminalLaunchProfileMainEventSchema = z.strictObject({
	requestId: DesktopTerminalLaunchProfileRequestIdSchema,
	result: desktopTerminalLaunchProfileResultSchema,
	type: z.literal('desktop-terminal-launch-profile-responded'),
});

export type DesktopTerminalLaunchProfile = z.infer<typeof DesktopTerminalLaunchProfileSchema>;
export type DesktopTerminalLaunchProfileCatalog = z.infer<
	typeof DesktopTerminalLaunchProfileCatalogSchema
>;

export type DesktopTerminalLaunchProfileFailureCode = z.infer<
	typeof DesktopTerminalLaunchProfileFailureCodeSchema
>;
export type DesktopTerminalLaunchProfileId = z.infer<typeof DesktopTerminalLaunchProfileIdSchema>;
export type DesktopTerminalLaunchProfileMainEvent = z.infer<
	typeof DesktopTerminalLaunchProfileMainEventSchema
>;
export type DesktopTerminalLaunchProfileSnapshot = z.infer<
	typeof DesktopTerminalLaunchProfileSnapshotSchema
>;
