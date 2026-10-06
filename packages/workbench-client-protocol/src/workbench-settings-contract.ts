import { z } from 'zod';
import { ConfigurationSnapshotSchema } from './workbench-configuration-contract';
import {
	SiteIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_NATIVE_SETTINGS_REQUEST_BYTES = 4096;
export const NativeSettingsSourceSchema = z.union([
	z.literal('missing'),
	z
		.string()
		.length(64)
		.regex(/^[0-9a-f]{64}$/u),
]);
export const NativeSettingsChangesSchema = z
	.strictObject({
		themeName: z.enum(['terminal-native', 'one-light', 'one-dark']).nullable(),
		backgroundReminders: z.boolean().nullable(),
		soundEnabled: z.boolean().nullable(),
	})
	.refine(
		(changes) =>
			changes.themeName !== null ||
			changes.backgroundReminders !== null ||
			changes.soundEnabled !== null,
	)
	.readonly();
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
				expectedRevision: WorkbenchRevisionSchema,
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
			z.strictObject({
				kind: z.literal('opened'),
				source: NativeSettingsSourceSchema,
				snapshot: ConfigurationSnapshotSchema,
			}),
			z.strictObject({ kind: z.literal('no-change'), snapshot: ConfigurationSnapshotSchema }),
			z.strictObject({ kind: z.literal('saved-applied'), snapshot: ConfigurationSnapshotSchema }),
			z.strictObject({
				kind: z.literal('saved-not-applied'),
				snapshot: ConfigurationSnapshotSchema,
			}),
			z.strictObject({
				kind: z.literal('refused'),
				reason: z.enum([
					'denied',
					'unavailable',
					'site-mismatch',
					'busy',
					'source-changed',
					'configuration-changed',
					'invalid-source',
					'invalid-changes',
					'write-failed',
					'revision-exhausted',
					'response-too-large',
				]),
			}),
		]),
	})
	.readonly();
export type NativeSettingsRequest = z.infer<typeof NativeSettingsRequestSchema>;
export type NativeSettingsResult = z.infer<typeof NativeSettingsResultSchema>;
