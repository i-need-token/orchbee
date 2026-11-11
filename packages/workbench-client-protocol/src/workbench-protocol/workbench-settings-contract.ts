import { z } from 'zod';

import { UserLaunchProfileCatalogSchema } from '../user-launch-profile-contract';
import { ConfigurationDiagnosticsSchema } from './workbench-configuration-contract';
import {
	SessionIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';

/** Encoded bound of one settings request or result. */
export const WORKBENCH_MAXIMUM_NATIVE_SETTINGS_REQUEST_BYTES = 2 * 1024 * 1024;
const MAXIMUM_SETTING_CHANGES = 64;
const MAXIMUM_SETTING_TEXT_BYTES = 4096;
const MAXIMUM_SETTING_KEY_BYTES = 128;
const MAXIMUM_SETTING_ENTRIES = 64;
const MAXIMUM_STRUCTURED_VALUE_BYTES = 64 * 1024;
const MAXIMUM_SNAPSHOT_KEYS = 1024;
const MAXIMUM_FILE_PATH_BYTES = 4096;
const MAXIMUM_POSITION = 4_294_967_295;

const utf8 = new TextEncoder();
/** C0 and C1 controls, which the Server never puts in a path it reports. */
const hasControlCharacter = (value: string): boolean =>
	[...value].some((character) => {
		const code = character.charCodeAt(0);
		return code < 32 || (code >= 127 && code <= 159);
	});
const byteLength = (value: string) => utf8.encode(value).byteLength;

/** A dotted setting path such as `terminal.font_size`. */
const SettingKeySchema = z
	.string()
	.min(1)
	.max(MAXIMUM_SETTING_KEY_BYTES)
	.regex(/^[a-z0-9_]+(?:\.[a-z0-9_]+)*$/u);
const TokenSchema = z
	.string()
	.min(1)
	.max(64)
	.regex(/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/u);
const PlainTextSchema = z.string().refine((value) => !value.includes('\0'));
const SettingTextSchema = PlainTextSchema.refine(
	(value) => byteLength(value) <= MAXIMUM_SETTING_TEXT_BYTES,
);
const PositionSchema = z.number().int().min(1).max(MAXIMUM_POSITION);
const IntegerSchema = z.number().int().refine(Number.isSafeInteger);

const SettingScalarSchema = z.union([z.boolean(), IntegerSchema, SettingTextSchema]);
const SettingEntrySchema = z.record(
	z
		.string()
		.max(MAXIMUM_SETTING_KEY_BYTES)
		.regex(/^[a-z0-9_]+$/u),
	z.union([z.boolean(), IntegerSchema, PlainTextSchema, z.array(PlainTextSchema).readonly()]),
);
/** A structured value in the file's form, such as one platform's launch profiles. */
const SettingEntriesSchema = z
	.array(SettingEntrySchema)
	.max(MAXIMUM_SETTING_ENTRIES)
	.readonly()
	.refine((entries) => byteLength(JSON.stringify(entries)) <= MAXIMUM_STRUCTURED_VALUE_BYTES);

const SettingChangeSchema = z.union([
	z
		.strictObject({
			key: SettingKeySchema,
			value: z.union([SettingScalarSchema, SettingEntriesSchema]),
		})
		.readonly(),
	z.strictObject({ key: SettingKeySchema, restore: z.literal(true) }).readonly(),
]);

const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	sessionId: SessionIdSchema,
};

export const SettingsRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.settings'),
		...correlation,
		operation: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('read') }).readonly(),
			z
				.strictObject({
					kind: z.literal('edit'),
					changes: z
						.array(SettingChangeSchema)
						.min(1)
						.max(MAXIMUM_SETTING_CHANGES)
						.readonly()
						.refine(
							(changes) => new Set(changes.map((change) => change.key)).size === changes.length,
						),
				})
				.readonly(),
		]),
	})
	.readonly();

const SettingsFileSchema = z.discriminatedUnion('status', [
	z.strictObject({ status: z.literal('ok') }).readonly(),
	z.strictObject({ status: z.literal('missing') }).readonly(),
	z
		.strictObject({
			status: z.literal('syntax-error'),
			reason: z.enum(['invalid-toml', 'invalid-utf8']),
			line: PositionSchema,
			column: PositionSchema,
		})
		.readonly(),
	z.strictObject({ status: z.literal('unreadable'), reason: TokenSchema }).readonly(),
]);

/** @internal Fixture parser for Web tests and Stories; production parses snapshots inside the settings result schema. */
export const SettingsSnapshotSchema = z
	.strictObject({
		revision: WorkbenchRevisionSchema,
		filePath: z
			.string()
			.min(1)
			.refine(
				(value) => byteLength(value) <= MAXIMUM_FILE_PATH_BYTES && !hasControlCharacter(value),
			)
			.nullable(),
		file: SettingsFileSchema,
		platform: z.enum(['windows', 'macos', 'linux']),
		values: z
			.record(SettingKeySchema, SettingScalarSchema.nullable())
			.refine((values) => Object.keys(values).length <= MAXIMUM_SNAPSHOT_KEYS)
			.readonly(),
		set: z
			.array(SettingKeySchema)
			.max(MAXIMUM_SNAPSHOT_KEYS)
			.readonly()
			.refine((keys) => keys.every((key, index) => index === 0 || (keys[index - 1] ?? '') < key)),
		launchProfiles: UserLaunchProfileCatalogSchema,
		diagnostics: ConfigurationDiagnosticsSchema,
	})
	.readonly();

const SETTINGS_REFUSAL_REASONS = [
	'denied',
	'unavailable',
	'session-mismatch',
	'busy',
	'read-only',
	'file-in-use',
	'locked',
	'changed',
	'too-large',
	'write-failed',
	'unknown-key',
	'invalid-value',
	'syntax-error',
	'unusable-file',
] as const;

const SettingsRefusalSchema = z
	.strictObject({
		outcome: z.literal('refused'),
		reason: z.enum(SETTINGS_REFUSAL_REASONS),
		key: SettingKeySchema.optional(),
		detail: TokenSchema.optional(),
		line: PositionSchema.optional(),
		column: PositionSchema.optional(),
	})
	.readonly()
	.refine((refusal) => {
		const members = {
			key: refusal.key !== undefined,
			detail: refusal.detail !== undefined,
			position: refusal.line !== undefined && refusal.column !== undefined,
			partialPosition: (refusal.line === undefined) !== (refusal.column === undefined),
		};
		if (members.partialPosition) return false;
		switch (refusal.reason) {
			case 'unknown-key':
				return members.key && !members.detail && !members.position;
			case 'invalid-value':
				return members.key && members.detail && !members.position;
			case 'syntax-error':
				return (
					!members.key &&
					(refusal.detail === 'invalid-toml' || refusal.detail === 'invalid-utf8') &&
					members.position
				);
			case 'unusable-file':
				return !members.key && members.detail && !members.position;
			default:
				return !members.key && !members.detail && !members.position;
		}
	});

const SettingsOutcomeSchema = z.union([
	z.strictObject({ outcome: z.literal('current'), snapshot: SettingsSnapshotSchema }).readonly(),
	z
		.strictObject({
			outcome: z.literal('edited'),
			snapshot: SettingsSnapshotSchema,
			changed: z.boolean(),
		})
		.readonly(),
	SettingsRefusalSchema,
]);

export const SettingsResultSchema = z
	.strictObject({
		kind: z.literal('workbench.settings.result'),
		...correlation,
		outcome: SettingsOutcomeSchema,
	})
	.readonly();

export type SettingChange = z.infer<typeof SettingChangeSchema>;
export type SettingEntries = z.infer<typeof SettingEntriesSchema>;
/** @internal The file state, for Web fixtures that publish one; production reads it from the snapshot. */
export type SettingsFile = z.infer<typeof SettingsFileSchema>;
export type SettingsSnapshot = z.infer<typeof SettingsSnapshotSchema>;
export type SettingsRefusal = z.infer<typeof SettingsRefusalSchema>;
export type SettingsRefusalReason = (typeof SETTINGS_REFUSAL_REASONS)[number];
