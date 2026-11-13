import { z } from 'zod';
import {
	SessionIdSchema,
	TerminalPresentationIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchPanelIdSchema,
	WorkbenchPresentationBindingSchema,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_ENCODED_PANEL_AGENT_RESULT_BYTES = 512 * 1024;
/** `opening`: the Agent process is seen and its screen is not yet recognised; only the Server publishes it. */
export const PanelAgentStateSchema = z.enum([
	'idle',
	'working',
	'blocked',
	'done',
	'error',
	'unknown',
	'opening',
]);
const PanelAgentReportStateSchema = PanelAgentStateSchema.exclude(['opening']);
const PanelAgentDispositionSchema = z.enum(['applied', 'ignored']);
const trim = (value: string) => value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '');
const label = z.string().refine((value) => trim(value).length > 0);
const normalizedLabel = label.refine((value) => trim(value) === value);
const optionalString = z
	.string()
	.nullish()
	.transform((value) => value ?? undefined);
const optionalSequence = WorkbenchRevisionSchema.nullish().transform((value) => value ?? undefined);
const PanelAgentReportSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('state'),
			source: z.string(),
			agent: label,
			state: PanelAgentReportStateSchema,
			message: optionalString,
			sequence: optionalSequence,
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
const PanelAgentActionSchema = z.discriminatedUnion('kind', [
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
			kind: z.literal('presentation-titles'),
			presentationIds: z
				.array(TerminalPresentationIdSchema)
				.max(64)
				.refine((ids) => new Set(ids).size === ids.length)
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
	z
		.strictObject({
			kind: z.literal('explain'),
			binding: TerminalAgentBindingSchema,
			inputArea: z.boolean().optional(),
		})
		.readonly(),
	z
		.strictObject({ kind: z.literal('identify'), presentationId: TerminalPresentationIdSchema })
		.readonly(),
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
/** Server-owned completion acknowledgement, independent of provider status and Client unread state. */
const AgentCompletionSchema = z.enum(['none', 'awaiting-input', 'acknowledged']);

export const PanelAgentViewSchema = z
	.strictObject({
		agent: normalizedLabel.nullable(),
		state: PanelAgentStateSchema,
		completion: AgentCompletionSchema.optional(),
		authoritySource: z.string().nullable(),
		message: z.string().nullable(),
	})
	.refine(
		(view) =>
			(view.authoritySource === null || view.agent !== null) &&
			(view.message === null || view.authoritySource !== null),
	)
	.readonly();
const PanelAgentRowSchema = z
	.strictObject({
		panelId: WorkbenchPanelIdSchema,
		binding: WorkbenchPresentationBindingSchema.nullable(),
		report: PanelAgentViewSchema.nullable(),
	})
	.refine((row) => (row.binding === null) === (row.report === null))
	.readonly();
const PanelAgentFailureSchema = z.enum([
	'denied',
	'unavailable',
	'session-mismatch',
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
			'atomcode',
			'codebuddy',
			'command-code',
			'deveco',
			'mimo',
			'reasonix',
			'snow',
			'minimax-code',
			'mothx',
			'jcode',
			'openclaw',
			'prime-agent',
		]),
		state: PanelAgentStateSchema,
		manifestSource: z.string().nullable(),
		manifest: AgentDetectionManifestSchema.optional(),
		manifestVersion: scalarBound(128).nullable(),
		matchedRule: detectionRule.nullable(),
		visibleIdle: z.boolean(),
		visibleBlocker: z.boolean(),
		visibleWorking: z.boolean(),
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
		let winner: z.infer<typeof detectionRule> | null = null;
		for (const evaluated of e.evaluatedRules) {
			if (evaluated.matched && (winner === null || evaluated.rule.priority > winner.priority))
				winner = evaluated.rule;
		}
		if (
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
			e.state !== (winner?.state ?? 'unknown') ||
			(e.visibleIdle && e.state !== 'idle') ||
			(e.visibleBlocker && e.state !== 'blocked') ||
			(e.visibleWorking && e.state !== 'working')
		)
			return false;
		if (winner === null)
			return (
				e.fallbackReason === 'no-matching-state-evidence' &&
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
const processAnchor = z
	.strictObject({
		pid: z.number().int().min(0).max(4_294_967_295),
		providerId: z.string().regex(/^[0-9A-Za-z._:-]{1,128}$/u),
	})
	.readonly();
const processIdentity = z.discriminatedUnion('result', [
	z
		.strictObject({
			result: z.literal('agent'),
			providerId: z.string().regex(/^[0-9A-Za-z._:-]{1,128}$/u),
			grade: z.enum(['confirmed', 'recognized']),
		})
		.readonly(),
	z.strictObject({ result: z.literal('none') }).readonly(),
	z
		.strictObject({
			result: z.literal('unknown'),
			reason: z.enum([
				'settling',
				'sample-failed',
				'sample-limit',
				'command-line-unreadable',
				'ambiguous-candidates',
				'unclassified-arguments',
				'unverified-provenance',
				'detached-console',
			]),
		})
		.readonly(),
	z
		.strictObject({
			result: z.literal('unsupported'),
			reason: z.enum(['platform', 'remote-execution']),
		})
		.readonly(),
]);
const terminalExplanation = z
	.strictObject({
		binding: TerminalAgentBindingSchema,
		identity: processIdentity,
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
			.readonly(),
		inputRow: z.int().min(0).max(65535).optional(),
	})
	.readonly();
const PresentationAgentRowSchema = z
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
/** A Server-published title: stripped, free of control characters, at most 1 KiB of UTF-8. */
const publishedTitle = z
	.string()
	.refine(
		(v) =>
			v.length > 0 &&
			!/[\p{Cc}\uD800-\uDFFF]/u.test(v) &&
			new TextEncoder().encode(v).length <= 1024,
	);
const presentationTitleRow = z
	.strictObject({
		presentationId: TerminalPresentationIdSchema,
		binding: WorkbenchPresentationBindingSchema.nullable(),
		title: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('unavailable') }).readonly(),
			z.strictObject({ kind: z.literal('observed'), title: publishedTitle.nullable() }).readonly(),
		]),
	})
	.refine((row) => row.binding !== null || row.title.kind === 'unavailable')
	.readonly();
const PanelAgentOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('presentation-titles'),
			presentations: z
				.array(presentationTitleRow)
				.max(64)
				.refine((rows) => new Set(rows.map((r) => r.presentationId)).size === rows.length)
				.readonly(),
		})
		.readonly(),
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
			kind: z.literal('identified'),
			workbenchRevision: WorkbenchRevisionSchema,
			binding: TerminalAgentBindingSchema,
			identity: processIdentity,
			anchor: z.union([z.null(), processAnchor]),
		})
		.refine(
			({ identity, anchor }) =>
				identity.result !== 'agent' || anchor?.providerId === identity.providerId,
		)
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
									completion: AgentCompletionSchema.optional(),
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
		sessionId: SessionIdSchema,
		action: PanelAgentActionSchema,
	})
	.readonly();
export const PanelAgentResultSchema = z
	.strictObject({
		kind: z.literal('workbench.panel-agent.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		sessionId: SessionIdSchema,
		outcome: PanelAgentOutcomeSchema,
	})
	.readonly();
