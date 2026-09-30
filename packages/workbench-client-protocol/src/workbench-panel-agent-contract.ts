import { z } from 'zod';
import {
	SiteIdSchema,
	TerminalPresentationIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchPanelIdSchema,
	WorkbenchPresentationBindingSchema,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_ENCODED_PANEL_AGENT_RESULT_BYTES = 512 * 1024;
export const PanelAgentStateSchema = z.enum(['idle', 'working', 'blocked', 'unknown']);
export const PanelAgentDispositionSchema = z.enum(['applied', 'ignored', 'deferred']);
const trim = (value: string) => value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '');
const label = z.string().refine((value) => trim(value).length > 0);
const normalizedLabel = label.refine((value) => trim(value) === value);
const optionalString = z
	.string()
	.nullish()
	.transform((value) => value ?? undefined);
const optionalSequence = WorkbenchRevisionSchema.nullish().transform((value) => value ?? undefined);
export const PanelAgentReportSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('state'),
			source: z.string(),
			agent: label,
			state: PanelAgentStateSchema,
			message: optionalString,
			sequence: optionalSequence,
			agentSessionId: optionalString,
			agentSessionPath: optionalString,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('session'),
			source: z.string(),
			agent: label,
			sequence: optionalSequence,
			agentSessionId: optionalString,
			agentSessionPath: optionalString,
			sessionStartSource: optionalString,
		})
		.readonly(),
	z
		.strictObject({ kind: z.literal('clear'), source: optionalString, sequence: optionalSequence })
		.readonly(),
	z
		.strictObject({
			kind: z.literal('release'),
			source: z.string(),
			agent: label,
			sequence: optionalSequence,
		})
		.readonly(),
]);
const hostIdentity = z
	.string()
	.regex(/^[0-9a-f]{32}$/u)
	.refine((s) => s !== '00000000000000000000000000000000');
export const TerminalAgentBindingSchema = z
	.strictObject({
		presentationId: TerminalPresentationIdSchema,
		generation: hostIdentity,
		hostEpoch: hostIdentity,
		hostTerminalId: hostIdentity,
	})
	.readonly();
export const PanelAgentActionSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('titles'),
			bindings: z
				.array(TerminalAgentBindingSchema)
				.max(4)
				.refine((rows) => new Set(rows.map((r) => r.presentationId)).size === rows.length)
				.readonly(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('read-presentations'),
			presentationIds: z
				.array(TerminalPresentationIdSchema)
				.max(4)
				.refine((ids) => new Set(ids).size === ids.length)
				.readonly(),
		})
		.readonly(),
	z.strictObject({ kind: z.literal('explain'), binding: TerminalAgentBindingSchema }).readonly(),
	z
		.strictObject({
			kind: z.literal('states'),
			presentationIds: z
				.array(TerminalPresentationIdSchema)
				.max(64)
				.refine((ids) => new Set(ids).size === ids.length)
				.readonly(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('observe'),
			binding: TerminalAgentBindingSchema,
			afterSequence: WorkbenchRevisionSchema.nullable(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('read'),
			panelIds: z
				.array(WorkbenchPanelIdSchema)
				.max(4)
				.refine((ids) => new Set(ids).size === ids.length)
				.readonly(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('report-presentation'),
			panelId: WorkbenchPanelIdSchema,
			presentationId: TerminalPresentationIdSchema,
			expectedBinding: WorkbenchPresentationBindingSchema,
			report: PanelAgentReportSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('report'),
			panelId: WorkbenchPanelIdSchema,
			expectedBinding: WorkbenchPresentationBindingSchema,
			report: PanelAgentReportSchema,
		})
		.readonly(),
]);
export const PanelAgentReferenceSchema = z
	.strictObject({ kind: z.enum(['id', 'path']), value: z.string() })
	.refine(
		(ref) =>
			ref.value.length > 0 &&
			!/\p{Cc}/u.test(ref.value) &&
			new TextEncoder().encode(ref.value).byteLength <= (ref.kind === 'id' ? 512 : 4096),
	)
	.readonly();
const officialPairs: ReadonlySet<string> = new Set([
	'herdr:pi/pi',
	'herdr:omp/omp',
	'herdr:mastracode/mastracode',
	'herdr:opencode/opencode',
	'herdr:kilo/kilo',
	'herdr:kimi/kimi',
	'herdr:claude/claude',
	'herdr:codex/codex',
	'herdr:copilot/copilot',
	'herdr:devin/devin',
	'herdr:droid/droid',
	'herdr:qodercli/qodercli',
	'herdr:cursor/cursor',
	'herdr:grok/grok',
	'herdr:hermes/hermes',
	'herdr:qwen/qwen',
	'herdr:letta/letta',
	'herdr:antigravity_cli/agy',
]);
export const PanelAgentSessionSchema = z
	.strictObject({
		source: z.string(),
		agent: normalizedLabel,
		reference: PanelAgentReferenceSchema,
	})
	.refine(
		(session) =>
			officialPairs.has(`${session.source}/${session.agent}`) &&
			(session.reference.kind === 'id' || session.agent === 'pi' || session.agent === 'omp'),
	)
	.readonly();
export const PanelAgentViewSchema = z
	.strictObject({
		agent: normalizedLabel.nullable(),
		state: PanelAgentStateSchema,
		authoritySource: z.string().nullable(),
		message: z.string().nullable(),
		session: PanelAgentSessionSchema.nullable(),
	})
	.refine(
		(view) =>
			(view.authoritySource === null || view.agent !== null) &&
			(view.message === null || view.authoritySource !== null),
	)
	.readonly();
export const PanelAgentRowSchema = z
	.strictObject({
		panelId: WorkbenchPanelIdSchema,
		binding: WorkbenchPresentationBindingSchema.nullable(),
		report: PanelAgentViewSchema.nullable(),
	})
	.refine((row) => (row.binding === null) === (row.report === null))
	.readonly();
export const PanelAgentFailureSchema = z.enum([
	'denied',
	'unavailable',
	'site-mismatch',
	'invalid-request',
	'not-found',
	'unbound',
	'binding-changed',
	'source-limit',
	'history-limit',
	'resource-limit',
	'revision-exhausted',
	'clock-overflow',
	'result-too-large',
]);
const semanticChange = z
	.strictObject({
		sequence: WorkbenchRevisionSchema.refine((s) => s !== '0'),
		agent: normalizedLabel.nullable(),
		state: PanelAgentStateSchema,
	})
	.readonly();
const semanticObservation = z
	.strictObject({
		binding: TerminalAgentBindingSchema,
		sequence: WorkbenchRevisionSchema,
		changes: z.array(semanticChange).max(128).readonly(),
		view: PanelAgentViewSchema,
	})
	.refine((o) => {
		const last = o.changes.at(-1);
		return (
			o.changes.every(
				(c, i) => i === 0 || BigInt(o.changes[i - 1]?.sequence ?? '0') + 1n === BigInt(c.sequence),
			) &&
			(last === undefined ||
				(last.sequence === o.sequence &&
					last.agent === o.view.agent &&
					last.state === o.view.state))
		);
	})
	.readonly();
const scalarBound = (maximum: number) => z.string().refine((s) => Array.from(s).length <= maximum);
const detectionRule = z
	.strictObject({
		id: scalarBound(128).refine((s) => trim(s).length > 0),
		priority: z.int().min(-2147483648).max(2147483647),
		region: label.refine((s) => new TextEncoder().encode(s).byteLength <= 128),
		state: PanelAgentStateSchema,
	})
	.readonly();
const detectionEvidence = z
	.strictObject({
		contains: z.array(scalarBound(512)).readonly(),
		regex: z.array(scalarBound(512)).readonly(),
		lineRegex: z.array(scalarBound(512)).readonly(),
		allCount: z.int().min(0).max(512),
		anyCount: z.int().min(0).max(512),
		notCount: z.int().min(0).max(512),
		regionBytes: z.int().min(0).max(65536),
		regionPreview: scalarBound(243),
	})
	.refine((e) => e.contains.length + e.regex.length + e.lineRegex.length <= 32)
	.readonly();
const sameRule = (
	a: z.infer<typeof detectionRule> | null,
	b: z.infer<typeof detectionRule> | null,
) =>
	a === null || b === null
		? a === b
		: a.id === b.id && a.priority === b.priority && a.region === b.region && a.state === b.state;
const manifestSource = z.enum(['bundled', 'local-override', 'remote-cache']);
export const AgentDetectionManifestSchema = z
	.strictObject({
		source: manifestSource,
		path: z
			.string()
			.refine(
				(s) =>
					s.length > 0 && new TextEncoder().encode(s).byteLength <= 32768 && !/\p{Cc}/u.test(s),
			)
			.nullable(),
		cachedRemoteVersion: z
			.string()
			.max(128)
			.refine(
				(s) =>
					/^[0-9]+(?:\.[0-9]+)*$/u.test(s) &&
					s.split('.').every((part) => BigInt(part) <= 18446744073709551615n),
			)
			.nullable(),
		localOverrideShadowingRemote: z.boolean(),
		warnings: z
			.array(
				z
					.strictObject({
						source: z.enum(['local-override', 'remote-cache']),
						kind: z.enum([
							'unreadable',
							'not-regular',
							'too-large',
							'invalid-utf8',
							'invalid-manifest',
							'invalid-version',
							'older-than-bundled',
						]),
					})
					.readonly(),
			)
			.max(2)
			.readonly(),
	})
	.refine(
		(m) =>
			(m.source === 'bundled' ? m.path === null : m.path !== null) &&
			(m.source !== 'remote-cache' || m.cachedRemoteVersion !== null) &&
			(!m.localOverrideShadowingRemote || m.cachedRemoteVersion !== null) &&
			new Set(m.warnings.map((w) => w.source)).size === m.warnings.length,
	)
	.readonly();
const screenExplanation = z
	.strictObject({
		agent: z.enum([
			'pi',
			'claude',
			'codex',
			'gemini',
			'cursor',
			'devin',
			'agy',
			'cline',
			'omp',
			'mastracode',
			'opencode',
			'copilot',
			'kimi',
			'kiro',
			'droid',
			'amp',
			'grok',
			'hermes',
			'kilo',
			'qodercli',
			'qwen',
			'letta',
			'maki',
			'muse',
		]),
		state: PanelAgentStateSchema,
		manifestSource: z.string().nullable(),
		manifest: AgentDetectionManifestSchema.optional(),
		manifestVersion: scalarBound(128).nullable(),
		matchedRule: detectionRule.nullable(),
		visibleIdle: z.boolean(),
		visibleBlocker: z.boolean(),
		visibleWorking: z.boolean(),
		screenDetectionSkipped: z.boolean(),
		screenDetectionSkipReason: z.string().nullable(),
		skipStateUpdate: z.boolean(),
		skippedUpdateReason: z.string().nullable(),
		fallbackReason: z.string().nullable(),
		evaluatedRules: z
			.array(
				z
					.strictObject({ rule: detectionRule, matched: z.boolean(), evidence: detectionEvidence })
					.readonly(),
			)
			.max(128)
			.readonly(),
	})
	.refine((e) => {
		const noFlags = !e.visibleIdle && !e.visibleBlocker && !e.visibleWorking;
		if (e.screenDetectionSkipped) {
			return (
				e.manifest === undefined &&
				e.screenDetectionSkipReason === 'full-lifecycle-hook-authority' &&
				e.manifestSource === null &&
				e.manifestVersion === null &&
				e.matchedRule === null &&
				e.evaluatedRules.length === 0 &&
				noFlags &&
				!e.skipStateUpdate &&
				e.skippedUpdateReason === null &&
				e.fallbackReason === null
			);
		}
		let winner: z.infer<typeof detectionRule> | null = null;
		for (const evaluated of e.evaluatedRules) {
			if (evaluated.matched && (winner === null || evaluated.rule.priority > winner.priority))
				winner = evaluated.rule;
		}
		if (
			e.screenDetectionSkipReason !== null ||
			!(
				(e.manifest !== undefined &&
					e.manifestSource === e.manifest.source &&
					(e.manifest.source !== 'remote-cache' ||
						e.manifestVersion === e.manifest.cachedRemoteVersion)) ||
				(e.manifest === undefined && e.manifestSource === 'bundled') ||
				(e.manifest === undefined &&
					e.manifestSource === null &&
					e.manifestVersion === null &&
					e.evaluatedRules.length === 0)
			) ||
			!sameRule(e.matchedRule, winner) ||
			e.state !== (winner?.state ?? 'idle') ||
			(e.visibleIdle && e.state !== 'idle') ||
			(e.visibleBlocker && e.state !== 'blocked') ||
			(e.visibleWorking && e.state !== 'working')
		)
			return false;
		if (winner === null)
			return (
				e.fallbackReason === 'default-known-agent-idle-fallback' &&
				noFlags &&
				!e.skipStateUpdate &&
				e.skippedUpdateReason === null
			);
		return (
			e.fallbackReason === null &&
			(e.skipStateUpdate
				? e.state === 'unknown' && noFlags && e.skippedUpdateReason === `matched-rule:${winner.id}`
				: e.skippedUpdateReason === null)
		);
	})
	.readonly();
const fullLifecyclePairs: ReadonlySet<string> = new Set([
	'herdr:pi/pi',
	'herdr:omp/omp',
	'herdr:mastracode/mastracode',
	'herdr:opencode/opencode',
	'herdr:kilo/kilo',
	'herdr:kimi/kimi',
]);
const terminalExplanation = z
	.strictObject({
		binding: TerminalAgentBindingSchema,
		view: PanelAgentViewSchema,
		explanation: screenExplanation,
		sample: z
			.strictObject({
				frameSequence: WorkbenchRevisionSchema.refine((s) => s !== '0'),
				frameGeneration: WorkbenchRevisionSchema.refine((s) => s !== '0'),
				frameRevision: WorkbenchRevisionSchema,
				rootPid: z.int().min(1).max(4294967295),
				agentPid: z.int().min(1).max(4294967295).nullable(),
			})
			.readonly()
			.nullable(),
	})
	.refine((t) =>
		t.explanation.screenDetectionSkipped
			? t.sample === null &&
				t.view.agent === t.explanation.agent &&
				t.view.state === t.explanation.state &&
				fullLifecyclePairs.has(`${t.view.authoritySource}/${t.explanation.agent}`)
			: t.sample !== null,
	)
	.readonly();
export const PresentationAgentRowSchema = z
	.strictObject({
		presentationId: TerminalPresentationIdSchema,
		panel: PanelAgentRowSchema,
	})
	.readonly();
const terminalTitleRow = z
	.strictObject({
		binding: TerminalAgentBindingSchema,
		observation: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('unavailable') }).readonly(),
			z
				.strictObject({
					kind: z.literal('observed'),
					revision: WorkbenchRevisionSchema.refine((v) => v !== '0'),
					title: z
						.string()
						.refine(
							(v) => !/[\uD800-\uDFFF]/u.test(v) && new TextEncoder().encode(v).length <= 64 * 1024,
						)
						.nullable(),
				})
				.readonly(),
		]),
	})
	.readonly();
export const PanelAgentOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('titles'),
			workbenchRevision: WorkbenchRevisionSchema,
			presentations: z
				.array(terminalTitleRow)
				.max(4)
				.refine((rows) => new Set(rows.map((r) => r.binding.presentationId)).size === rows.length)
				.readonly(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('presentations-read'),
			workbenchRevision: WorkbenchRevisionSchema,
			reportRevision: WorkbenchRevisionSchema,
			presentations: z
				.array(PresentationAgentRowSchema)
				.max(4)
				.refine((rows) => new Set(rows.map((r) => r.presentationId)).size === rows.length)
				.readonly(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('explained'),
			workbenchRevision: WorkbenchRevisionSchema,
			reportRevision: WorkbenchRevisionSchema,
			explanation: terminalExplanation,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('states'),
			workbenchRevision: WorkbenchRevisionSchema,
			reportRevision: WorkbenchRevisionSchema,
			presentations: z
				.array(
					z
						.strictObject({
							presentationId: TerminalPresentationIdSchema,
							snapshot: z
								.strictObject({
									binding: TerminalAgentBindingSchema,
									sequence: WorkbenchRevisionSchema,
									agent: normalizedLabel.nullable(),
									state: PanelAgentStateSchema,
								})
								.readonly()
								.nullable(),
						})
						.refine(
							(row) =>
								row.snapshot === null || row.snapshot.binding.presentationId === row.presentationId,
						)
						.readonly(),
				)
				.max(64)
				.refine((rows) => new Set(rows.map((r) => r.presentationId)).size === rows.length)
				.readonly(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('observed'),
			workbenchRevision: WorkbenchRevisionSchema,
			reportRevision: WorkbenchRevisionSchema,
			observation: semanticObservation,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('read'),
			workbenchRevision: WorkbenchRevisionSchema,
			reportRevision: WorkbenchRevisionSchema,
			panels: z
				.array(PanelAgentRowSchema)
				.max(4)
				.refine((panels) => new Set(panels.map((p) => p.panelId)).size === panels.length)
				.readonly(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('presentation-reported'),
			workbenchRevision: WorkbenchRevisionSchema,
			reportRevision: WorkbenchRevisionSchema,
			panelId: WorkbenchPanelIdSchema,
			presentationId: TerminalPresentationIdSchema,
			binding: WorkbenchPresentationBindingSchema,
			disposition: PanelAgentDispositionSchema,
			changed: z.boolean(),
			view: PanelAgentViewSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('reported'),
			workbenchRevision: WorkbenchRevisionSchema,
			reportRevision: WorkbenchRevisionSchema,
			panelId: WorkbenchPanelIdSchema,
			binding: WorkbenchPresentationBindingSchema,
			disposition: PanelAgentDispositionSchema,
			changed: z.boolean(),
			view: PanelAgentViewSchema,
		})
		.readonly(),
	z.strictObject({ kind: z.literal('failed'), reason: PanelAgentFailureSchema }).readonly(),
]);
export const PanelAgentRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.panel-agent'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		siteId: SiteIdSchema,
		action: PanelAgentActionSchema,
	})
	.readonly();
export const PanelAgentResultSchema = z
	.strictObject({
		kind: z.literal('workbench.panel-agent.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		siteId: SiteIdSchema,
		outcome: PanelAgentOutcomeSchema,
	})
	.readonly();

export type PanelAgentState = z.infer<typeof PanelAgentStateSchema>;
export type PanelAgentDisposition = z.infer<typeof PanelAgentDispositionSchema>;
export type PanelAgentReport = z.infer<typeof PanelAgentReportSchema>;
export type PanelAgentAction = z.infer<typeof PanelAgentActionSchema>;
export type PanelAgentReference = z.infer<typeof PanelAgentReferenceSchema>;
export type PanelAgentSession = z.infer<typeof PanelAgentSessionSchema>;
export type PanelAgentView = z.infer<typeof PanelAgentViewSchema>;
export type PanelAgentRow = z.infer<typeof PanelAgentRowSchema>;
export type PanelAgentFailure = z.infer<typeof PanelAgentFailureSchema>;
export type PanelAgentOutcome = z.infer<typeof PanelAgentOutcomeSchema>;
export type PanelAgentRequest = z.infer<typeof PanelAgentRequestSchema>;
export type PanelAgentResult = z.infer<typeof PanelAgentResultSchema>;
export type PresentationAgentRow = z.infer<typeof PresentationAgentRowSchema>;
