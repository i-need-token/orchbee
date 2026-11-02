import { TerminalConfigurationSchema } from '@orchbee/terminal-protocol';
import { UserPreferencesSourceSchema } from '@orchbee/workbench-client-protocol';
import { z } from 'zod';

const DesktopTerminalDefaultSettingsPersistenceIssueSchema = z.enum([
	'invalid-record',
	'read-failed',
	'write-failed',
]);

/** @internal Fixture parser for the Web terminal settings Stories; no production boundary parses it. */
export const DesktopTerminalDefaultSettingsSnapshotSchema = z.strictObject({
	preferencesSource: UserPreferencesSourceSchema.optional(),
	configuration: TerminalConfigurationSchema,
	persistenceIssue: DesktopTerminalDefaultSettingsPersistenceIssueSchema.nullable(),
	source: z.enum(['runtime', 'stored', 'session']),
});

const DesktopTerminalDefaultSettingsRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopTerminalDefaultSettingsRequestId'>();

const DesktopTerminalDefaultSettingsFailureCodeSchema = z.enum([
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

const DesktopTerminalDefaultSettingsMainEventSchema = z.strictObject({
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
export type DesktopTerminalDefaultSettingsSnapshot = z.infer<
	typeof DesktopTerminalDefaultSettingsSnapshotSchema
>;
