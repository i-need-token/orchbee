import { z } from 'zod';
import {
	SessionIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_ENCODED_CONFIGURATION_BYTES = 256 * 1024;
const MAXIMUM_NOTIFICATION_PROFILE_BYTES = 16 * 1024;
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
const compareUnicodeScalars = (left: string, right: string) => {
	const leftScalars = [...left];
	const rightScalars = [...right];
	for (let index = 0; index < Math.min(leftScalars.length, rightScalars.length); index += 1) {
		const leftCodePoint = leftScalars[index]?.codePointAt(0) ?? -1;
		const rightCodePoint = rightScalars[index]?.codePointAt(0) ?? -1;
		if (leftCodePoint !== rightCodePoint) return leftCodePoint - rightCodePoint;
	}
	return leftScalars.length - rightScalars.length;
};
const ConfigurationDiagnosticSchema = z
	.strictObject({
		field: bounded(256).refine((value) => !/\p{Cc}/u.test(value)),
		reason: z
			.string()
			.max(64)
			.regex(/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/u),
		disposition: z.enum(['defaulted', 'disabled', 'ignored']),
	})
	.readonly();
export const ConfigurationDiagnosticsSchema = z
	.strictObject({
		items: z.array(ConfigurationDiagnosticSchema).max(128).readonly(),
		omittedCount: z.number().int().min(0).max(65536),
	})
	.refine((diagnostics) =>
		diagnostics.items.every((item, index) => {
			const previous = diagnostics.items[index - 1];
			if (previous === undefined) return true;
			const fieldOrder = compareUnicodeScalars(previous.field, item.field);
			return (
				fieldOrder < 0 ||
				(fieldOrder === 0 && compareUnicodeScalars(previous.reason, item.reason) < 0)
			);
		}),
	)
	.readonly();
const configuredPath = bounded(4096).refine(
	(value) => /^(?:\/|(?:\\\\\?\\)?[A-Za-z]:[/\\])/u.test(value) && !/\p{Cc}/u.test(value),
);
const ConfigurationSnapshotSchema = z.strictObject({
	revision: WorkbenchRevisionSchema,
	diagnostics: ConfigurationDiagnosticsSchema,
	keysProfile: bounded(65536),
	windowTitle: bounded(4096, 0),
	notificationProfile: bounded(MAXIMUM_NOTIFICATION_PROFILE_BYTES),
	agentSidebarProfile: bounded(32768),
	spacesSidebarProfile: bounded(32768),
	manifestCheck: z.boolean(),
	focusNewPanel: z.boolean(),
	worktreesDirectory: configuredPath.nullable(),
	newCwd: z.union([z.enum(['follow', 'home', 'current', 'workspace']), configuredPath]),
});
const failure = z
	.strictObject({
		kind: z.literal('failed'),
		reason: z.enum([
			'denied',
			'unavailable',
			'session-mismatch',
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
	sessionId: SessionIdSchema,
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
			z.strictObject({ kind: z.literal('current'), snapshot: ConfigurationSnapshotSchema }),
			z.strictObject({
				kind: z.literal('reloaded'),
				snapshot: ConfigurationSnapshotSchema,
				changed: z.boolean(),
			}),
			failure,
		]),
	})
	.readonly();
