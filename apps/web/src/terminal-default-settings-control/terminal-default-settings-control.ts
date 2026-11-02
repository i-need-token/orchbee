import {
	TerminalConfigurationSchema,
	type TerminalConfiguration,
} from '@orchbee/terminal-protocol';
import { UserPreferencesSourceSchema } from '@orchbee/workbench-client-protocol';
import { z } from 'zod';

const TerminalDefaultSettingsPersistenceIssueSchema = z.enum([
	'invalid-record',
	'read-failed',
	'write-failed',
]);

/** @internal Fixture parser for the Web terminal settings Stories; no production boundary parses it. */
export const TerminalDefaultSettingsSnapshotSchema = z.strictObject({
	preferencesSource: UserPreferencesSourceSchema.optional(),
	configuration: TerminalConfigurationSchema,
	persistenceIssue: TerminalDefaultSettingsPersistenceIssueSchema.nullable(),
	source: z.enum(['runtime', 'stored', 'session']),
});

const TerminalDefaultSettingsFailureCodeSchema = z.enum([
	'busy',
	'control-unavailable',
	'host-rejected',
	'write-failed',
	'source-changed',
	'outcome-unknown',
]);

const TerminalDefaultSettingsResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('completed'),
		snapshot: TerminalDefaultSettingsSnapshotSchema,
	}),
	z.strictObject({
		code: TerminalDefaultSettingsFailureCodeSchema,
		kind: z.literal('failed'),
		snapshot: TerminalDefaultSettingsSnapshotSchema,
	}),
]);

export type TerminalDefaultSettingsFailureCode = z.infer<
	typeof TerminalDefaultSettingsFailureCodeSchema
>;
export type TerminalDefaultSettingsSnapshot = z.infer<typeof TerminalDefaultSettingsSnapshotSchema>;

export interface TerminalDefaultSettingsClientSnapshot extends TerminalDefaultSettingsSnapshot {
	readonly failureCode: TerminalDefaultSettingsFailureCode | null;
	readonly pending: boolean;
}

export type TerminalDefaultSettingsClientOperationResult =
	| z.infer<typeof TerminalDefaultSettingsResultSchema>
	| { readonly kind: 'unavailable' };

export interface TerminalDefaultSettingsControl {
	getSnapshot(): TerminalDefaultSettingsClientSnapshot;
	subscribe(listener: () => void): () => void;
	update(
		configuration: TerminalConfiguration,
		preferencesSource?: string,
	): Promise<TerminalDefaultSettingsClientOperationResult>;
}
