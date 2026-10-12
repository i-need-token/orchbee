import { TerminalConfigurationSchema } from '@orchbee/terminal-protocol';
import { UserPreferencesSourceSchema } from '@orchbee/workbench-client-protocol';
import { z } from 'zod';

export const DesktopTerminalDefaultSettingsPersistenceIssueSchema = z.enum([
	'invalid-record',
	'read-failed',
	'write-failed',
]);

export const DesktopTerminalDefaultSettingsSnapshotSchema = z.strictObject({
	preferencesSource: UserPreferencesSourceSchema.optional(),
	configuration: TerminalConfigurationSchema,
	persistenceIssue: DesktopTerminalDefaultSettingsPersistenceIssueSchema.nullable(),
	source: z.enum(['runtime', 'stored', 'session']),
});

export const DesktopTerminalDefaultSettingsRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopTerminalDefaultSettingsRequestId'>();

const requestIdentityShape = {
	preferencesSource: UserPreferencesSourceSchema.optional(),
	requestId: DesktopTerminalDefaultSettingsRequestIdSchema,
} as const;

export const DesktopTerminalDefaultSettingsRendererCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		...requestIdentityShape,
		type: z.literal('desktop-terminal-default-settings-snapshot-requested'),
	}),
	z.strictObject({
		...requestIdentityShape,
		configuration: TerminalConfigurationSchema,
		type: z.literal('desktop-terminal-default-settings-update-requested'),
	}),
]);

export const DesktopTerminalDefaultSettingsFailureCodeSchema = z.enum([
	'busy',
	'control-unavailable',
	'host-rejected',
	'write-failed',
	'source-changed',
	'outcome-unknown',
]);

const desktopTerminalDefaultSettingsResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('completed'),
		snapshot: DesktopTerminalDefaultSettingsSnapshotSchema,
	}),
	z.strictObject({
		code: DesktopTerminalDefaultSettingsFailureCodeSchema,
		kind: z.literal('failed'),
		snapshot: DesktopTerminalDefaultSettingsSnapshotSchema,
	}),
]);

export const DesktopTerminalDefaultSettingsMainEventSchema = z.strictObject({
	requestId: DesktopTerminalDefaultSettingsRequestIdSchema,
	result: desktopTerminalDefaultSettingsResultSchema,
	type: z.literal('desktop-terminal-default-settings-responded'),
});

export type DesktopTerminalDefaultSettingsFailureCode = z.infer<
	typeof DesktopTerminalDefaultSettingsFailureCodeSchema
>;
export type DesktopTerminalDefaultSettingsMainEvent = z.infer<
	typeof DesktopTerminalDefaultSettingsMainEventSchema
>;
export type DesktopTerminalDefaultSettingsPersistenceIssue = z.infer<
	typeof DesktopTerminalDefaultSettingsPersistenceIssueSchema
>;
export type DesktopTerminalDefaultSettingsRendererCommand = z.infer<
	typeof DesktopTerminalDefaultSettingsRendererCommandSchema
>;
export type DesktopTerminalDefaultSettingsRequestId = z.infer<
	typeof DesktopTerminalDefaultSettingsRequestIdSchema
>;
export type DesktopTerminalDefaultSettingsSnapshot = z.infer<
	typeof DesktopTerminalDefaultSettingsSnapshotSchema
>;
