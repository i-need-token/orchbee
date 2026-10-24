import { z } from 'zod';

import { UserLaunchProfileCatalogSchema } from './user-launch-profile-contract';

export const MAXIMUM_USER_PREFERENCES_BYTES = 2 * 1024 * 1024;
export const USER_PREFERENCE_KEYS = [
	'locale',
	'applicationTheme',
	'terminalDisplayTheme',
	'customForeground',
	'customBackground',
	'copyOnSelect',
	'trimTrailingBlankRowsForAgents',
	'agentNotificationMode',
	'completionReminderMode',
	'completionReminderMinimumSeconds',
	'soundEnabled',
	'scrollbackBytes',
	'launchProfiles',
] as const;
const UserPreferenceKeySchema = z.enum(USER_PREFERENCE_KEYS);
const locale = z.enum(['system', 'en-US', 'zh-CN']);
const applicationTheme = z.enum(['system', 'light', 'dark']);
const terminalDisplayTheme = z.enum(['host', 'light', 'dark', 'custom']);
const color = z.string().regex(/^#[\da-f]{6}$/u);
const agentNotificationMode = z.enum(['never', 'hidden']);
const completionReminderMode = z.enum(['never', 'unfocused', 'always']);
const completionReminderMinimumSeconds = z.number().int().min(1).max(3600);
const scrollbackBytes = z.number().int().min(0).max(1_000_000_000);

export const UserPreferencesValuesSchema = z.strictObject({
	locale,
	applicationTheme,
	terminalDisplayTheme,
	customForeground: color,
	customBackground: color,
	copyOnSelect: z.boolean(),
	trimTrailingBlankRowsForAgents: z.boolean(),
	agentNotificationMode,
	completionReminderMode,
	completionReminderMinimumSeconds,
	soundEnabled: z.boolean(),
	scrollbackBytes,
	launchProfiles: UserLaunchProfileCatalogSchema,
});

export const DEFAULT_USER_PREFERENCES: Readonly<UserPreferencesValues> = Object.freeze(
	UserPreferencesValuesSchema.parse({
		locale: 'system',
		applicationTheme: 'system',
		terminalDisplayTheme: 'host',
		customForeground: '#141414',
		customBackground: '#fcfcfc',
		copyOnSelect: true,
		trimTrailingBlankRowsForAgents: true,
		agentNotificationMode: 'never',
		completionReminderMode: 'never',
		completionReminderMinimumSeconds: 5,
		soundEnabled: false,
		scrollbackBytes: 10_000_000,
		launchProfiles: {
			version: 1,
			defaultProfileId: 'system-default',
			profiles: [
				{ id: 'system-default', name: null, command: { kind: 'system-shell', login: false } },
			],
		},
	}),
);

/** Null resets a key to its common default; absence preserves its current value. */
export const UserPreferencesPatchSchema = z.strictObject({
	locale: locale.nullable().optional(),
	applicationTheme: applicationTheme.nullable().optional(),
	terminalDisplayTheme: terminalDisplayTheme.nullable().optional(),
	customForeground: color.nullable().optional(),
	customBackground: color.nullable().optional(),
	copyOnSelect: z.boolean().nullable().optional(),
	trimTrailingBlankRowsForAgents: z.boolean().nullable().optional(),
	agentNotificationMode: agentNotificationMode.nullable().optional(),
	completionReminderMode: completionReminderMode.nullable().optional(),
	completionReminderMinimumSeconds: completionReminderMinimumSeconds.nullable().optional(),
	soundEnabled: z.boolean().nullable().optional(),
	scrollbackBytes: scrollbackBytes.nullable().optional(),
	launchProfiles: UserLaunchProfileCatalogSchema.nullable().optional(),
});

export const UserPreferencesSourceSchema = z.string().regex(/^(?:missing|[\da-f]{64})$/u);
export const UserPreferencesSnapshotSchema = z.strictObject({
	source: UserPreferencesSourceSchema,
	values: UserPreferencesValuesSchema,
	claimed: z
		.array(UserPreferenceKeySchema)
		.max(USER_PREFERENCE_KEYS.length)
		.refine((keys) => new Set(keys).size === keys.length, 'Claimed keys must be unique'),
});
const UserPreferencesFailureSchema = z.enum([
	'invalid-request',
	'invalid-changes',
	'invalid-source',
	'unavailable',
	'busy',
	'source-changed',
	'write-failed',
]);
const encoder = new TextEncoder();
export const UserPreferencesRequestSchema = z
	.discriminatedUnion('operation', [
		z.strictObject({ operation: z.literal('read') }),
		z.strictObject({
			operation: z.literal('update'),
			source: UserPreferencesSourceSchema,
			changes: UserPreferencesPatchSchema.refine(
				(changes) => Object.values(changes).some((value) => value !== undefined),
				'Update requires a changed key',
			),
		}),
	])
	.refine(
		(request) =>
			encoder.encode(JSON.stringify(request)).byteLength <= MAXIMUM_USER_PREFERENCES_BYTES,
		'Preference request exceeds its byte budget',
	);
export const UserPreferencesResponseSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('snapshot'),
		snapshot: UserPreferencesSnapshotSchema,
		changed: z.boolean(),
	}),
	z.strictObject({ kind: z.literal('refused'), reason: UserPreferencesFailureSchema }),
	z.strictObject({ kind: z.literal('outcome-unknown') }),
]);

export type UserPreferenceKey = z.infer<typeof UserPreferenceKeySchema>;
type UserPreferencesValues = z.infer<typeof UserPreferencesValuesSchema>;
export type UserPreferencesPatch = z.infer<typeof UserPreferencesPatchSchema>;
export type UserPreferencesSnapshot = z.infer<typeof UserPreferencesSnapshotSchema>;
export type UserPreferencesFailure = z.infer<typeof UserPreferencesFailureSchema>;
export type UserPreferencesRequest = z.infer<typeof UserPreferencesRequestSchema>;
export type UserPreferencesResponse = z.infer<typeof UserPreferencesResponseSchema>;
