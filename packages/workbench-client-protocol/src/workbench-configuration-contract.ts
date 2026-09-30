import { z } from 'zod';
import {
	SiteIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_ENCODED_CONFIGURATION_BYTES = 128 * 1024;
const bounded = (maximum: number, minimum = 1) =>
	z
		.string()
		.min(minimum)
		.max(maximum)
		.refine(
			(value) =>
				!/[\uD800-\uDFFF]/u.test(value) && new TextEncoder().encode(value).byteLength <= maximum,
		);
const diagnostic = bounded(128)
	.refine((value) => !/\p{Cc}/u.test(value))
	.nullable();
const snapshot = z.strictObject({
	revision: WorkbenchRevisionSchema,
	keysProfile: bounded(65536),
	windowTitle: bounded(4096, 0),
	notificationProfile: bounded(1024),
	agentSidebarProfile: bounded(32768),
	manifestCheck: z.boolean(),
	defaultShell: bounded(4096, 0).refine((value) => !/\p{Cc}/u.test(value)),
	shellMode: z.enum(['auto', 'login', 'non_login']),
	restartRequired: z
		.array(
			z.enum(['terminal.default_shell', 'terminal.shell_mode', 'session.resume_agents_on_restore']),
		)
		.max(3)
		.refine((values) => new Set(values).size === values.length)
		.readonly(),
});
const failure = z
	.strictObject({
		kind: z.literal('failed'),
		reason: z.enum([
			'denied',
			'unavailable',
			'site-mismatch',
			'busy',
			'load-failed',
			'revision-exhausted',
		]),
		field: diagnostic,
		detail: diagnostic,
	})
	.refine(
		(value) =>
			(value.field === null) === (value.detail === null) &&
			(value.reason === 'load-failed' || value.field === null),
	);
const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	siteId: SiteIdSchema,
};
export const ConfigurationRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.configuration'),
		...correlation,
		action: z.enum(['current', 'reload']),
	})
	.readonly();
export const ConfigurationResultSchema = z
	.strictObject({
		kind: z.literal('workbench.configuration.result'),
		...correlation,
		outcome: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('current'), snapshot }),
			z.strictObject({ kind: z.literal('reloaded'), snapshot, changed: z.boolean() }),
			failure,
		]),
	})
	.readonly();
export type ConfigurationRequest = z.infer<typeof ConfigurationRequestSchema>;
export type ConfigurationResult = z.infer<typeof ConfigurationResultSchema>;
