import { z } from 'zod';
import {
	MAXIMUM_USER_PREFERENCES_BYTES,
	UserPreferencesPatchSchema,
	UserPreferencesSnapshotSchema,
	UserPreferencesSourceSchema,
} from './user-preferences-contract';
import {
	SiteIdSchema,
	WorkbenchRequestIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_NATIVE_SETTINGS_REQUEST_BYTES = MAXIMUM_USER_PREFERENCES_BYTES;
export const NativeSettingsSourceSchema = UserPreferencesSourceSchema;
export const NativeSettingsChangesSchema = UserPreferencesPatchSchema.refine(
	(changes) => Object.keys(changes).length > 0,
);
const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	siteId: SiteIdSchema,
};
export const NativeSettingsRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.settings'),
		...correlation,
		operation: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('open') }),
			z.strictObject({
				kind: z.literal('save'),
				source: NativeSettingsSourceSchema,
				changes: NativeSettingsChangesSchema,
			}),
		]),
	})
	.readonly();
export const NativeSettingsResultSchema = z
	.strictObject({
		kind: z.literal('workbench.settings.result'),
		...correlation,
		outcome: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('opened'), snapshot: UserPreferencesSnapshotSchema }),
			z.strictObject({
				kind: z.literal('saved'),
				snapshot: UserPreferencesSnapshotSchema,
				changed: z.boolean(),
			}),
			z.strictObject({ kind: z.literal('outcome-unknown') }),
			z.strictObject({
				kind: z.literal('refused'),
				reason: z.enum([
					'denied',
					'unavailable',
					'site-mismatch',
					'busy',
					'source-changed',
					'invalid-source',
					'invalid-changes',
					'write-failed',
					'response-too-large',
				]),
			}),
		]),
	})
	.readonly();
export type NativeSettingsRequest = z.infer<typeof NativeSettingsRequestSchema>;
export type NativeSettingsResult = z.infer<typeof NativeSettingsResultSchema>;
