import { z } from 'zod';
import { WorkbenchRevisionSchema } from './workbench-semantic-contract';
const revision = WorkbenchRevisionSchema.refine((v) => v !== '0');
const AgentManifestUpdateFailureSchema = z.enum([
	'invalid-source',
	'invalid-catalog',
	'fetch',
	'too-large',
	'invalid-utf8',
	'invalid-manifest',
	'cache',
	'busy',
	'older-version',
	'changed-same-version',
	'cancelled',
	'deadline',
	'clock',
	'process',
]);
const source = z
	.string()
	.max(4096)
	.refine((value) => {
		if (!/^[\x21-\x7e]+$/u.test(value) || /[\\#?]/u.test(value)) return false;
		let tail: string;
		if (value.startsWith('https://')) tail = value.slice(8);
		else if (value.startsWith('http://')) tail = value.slice(7);
		else return false;
		const slash = tail.indexOf('/');
		return (
			slash > 0 &&
			tail
				.slice(0, slash)
				.split('')
				.every((c) => /[A-Za-z0-9]/u.test(c) || '.-:[]'.includes(c))
		);
	});
const version = z
	.string()
	.max(128)
	.refine(
		(s) =>
			/^[0-9]+(?:\.[0-9]+)*$/u.test(s) &&
			s.split('.').every((p) => BigInt(p) <= 18446744073709551615n),
	)
	.nullable();
const provider = z
	.strictObject({
		agent: z
			.string()
			.min(1)
			.max(64)
			.regex(/^[a-z0-9-]+$/u),
		attemptedVersion: version,
		cachedVersion: version,
		result: z.enum(['updated', 'current', 'failed']),
		error: AgentManifestUpdateFailureSchema.nullable(),
	})
	.refine((row) =>
		row.result === 'failed'
			? row.error !== null
			: row.error === null &&
				row.cachedVersion !== null &&
				row.attemptedVersion === row.cachedVersion,
	)
	.readonly();
const AgentManifestUpdateReportSchema = z
	.strictObject({
		version: z.literal(1),
		catalogUrl: source,
		checkedUnixSeconds: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER),
		agents: z
			.array(provider)
			.max(22)
			.refine((rows) => new Set(rows.map((r) => r.agent)).size === rows.length)
			.readonly(),
		failure: AgentManifestUpdateFailureSchema.nullable(),
		statusSaved: z.boolean(),
	})
	.readonly();
export const AgentManifestsFailureSchema = z.enum([
	'denied',
	'unavailable',
	'site-mismatch',
	'busy',
	'resource-limit',
	'not-found',
	'load-failed',
	'revision-exhausted',
	'cancelled',
	'conflict',
	'update-unavailable',
]);
const AgentManifestActivationSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('not-attempted') }),
	z.strictObject({ kind: z.literal('applied'), revision }),
	z.strictObject({ kind: z.literal('failed'), reason: AgentManifestsFailureSchema }),
]);
export const AgentManifestsOperationSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('pending') }),
	z.strictObject({
		kind: z.literal('updated'),
		report: AgentManifestUpdateReportSchema,
		activation: AgentManifestActivationSchema,
	}),
	z.strictObject({ kind: z.literal('unknown'), reason: AgentManifestsFailureSchema }),
	z.strictObject({ kind: z.literal('applied'), revision }),
	z.strictObject({ kind: z.literal('failed'), reason: AgentManifestsFailureSchema }),
]);
const AgentManifestAutomaticSchema = z
	.strictObject({
		manifestCheck: z.boolean(),
		manifestCheckSource: z.enum(['default', 'file']),
		backgroundEnabled: z.boolean(),
		releaseBuild: z.boolean(),
		updateAvailable: z.boolean(),
		enabled: z.boolean(),
		lastOperation: AgentManifestsOperationSchema.refine((o) => o.kind !== 'applied').nullable(),
	})
	.refine(
		(a) =>
			a.enabled ===
				(a.manifestCheck && a.backgroundEnabled && a.releaseBuild && a.updateAvailable) &&
			(a.manifestCheck || a.manifestCheckSource !== 'default'),
	)
	.readonly();
export const AgentManifestMaintenanceSchema = z
	.strictObject({
		sourceUrl: source.nullable(),
		automatic: AgentManifestAutomaticSchema.optional(),
		sourceError: AgentManifestUpdateFailureSchema.nullable(),
		lastUpdate: AgentManifestUpdateReportSchema.nullable(),
		statusError: AgentManifestUpdateFailureSchema.nullable(),
	})
	.refine((m) =>
		m.sourceUrl === null ? m.sourceError === 'invalid-source' : m.sourceError === null,
	)
	.readonly();
