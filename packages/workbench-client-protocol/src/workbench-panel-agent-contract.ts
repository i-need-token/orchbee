import { z } from 'zod';
import {
	SiteIdSchema,
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
export const PanelAgentActionSchema = z.discriminatedUnion('kind', [
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
export const PanelAgentOutcomeSchema = z.discriminatedUnion('kind', [
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
