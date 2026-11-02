import { z } from 'zod';
import {
	SessionIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkerIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';

/** One 8 KiB Agent integration request plus its job envelope. */
export const WORKBENCH_MAXIMUM_ENCODED_AGENT_INTEGRATION_REQUEST_BYTES = 10 * 1024;
/** One 32 KiB Agent integration report plus its job envelope. */
export const WORKBENCH_MAXIMUM_ENCODED_AGENT_INTEGRATION_RESULT_BYTES = 36 * 1024;
const MAXIMUM_PATH_UNITS = 4096;
const MAXIMUM_BUDGET_MILLISECONDS = 40_000;

const utf8 = new TextEncoder();

/** Provider-registry spellings in registry order, the only integration vocabulary on the wire. */
export const AGENT_INTEGRATION_PROVIDERS = [
	'claude-code',
	'codex',
	'grok',
	'opencode',
	'kilo',
	'cursor',
	'qodercli',
	'antigravity',
	'mastracode',
	'droid',
	'kimi',
	'letta',
	'devin',
	'hermes',
	'pi',
	'omp',
	'gemini',
	'github-copilot',
	'cline',
	'qwen-code',
] as const;

const ProviderSchema = z.enum(AGENT_INTEGRATION_PROVIDERS);

/** A path on the executing node. Only that node's platform family rebuilds a non-text form. */
const NodePathSchema = z.union([
	z
		.strictObject({
			text: z
				.string()
				.refine(
					(value) =>
						!value.includes('\0') &&
						!/[\uD800-\uDFFF]/u.test(value) &&
						utf8.encode(value).byteLength <= MAXIMUM_PATH_UNITS,
				),
		})
		.readonly(),
	z
		.strictObject({
			unixBytesHex: z
				.string()
				.max(MAXIMUM_PATH_UNITS * 2)
				.regex(/^(?:(?!00)[0-9A-Fa-f]{2})+$/u),
		})
		.readonly(),
	z
		.strictObject({
			windowsUtf16: z
				.array(z.number().int().min(1).max(0xff_ff))
				.min(1)
				.max(MAXIMUM_PATH_UNITS)
				.readonly(),
		})
		.readonly(),
]);

const OperationSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('list'), provider: ProviderSchema.nullish() }).readonly(),
	z.strictObject({ kind: z.literal('explain'), provider: ProviderSchema }).readonly(),
	z.strictObject({ kind: z.literal('enable'), provider: ProviderSchema }).readonly(),
	z.strictObject({ kind: z.literal('disable'), provider: ProviderSchema }).readonly(),
]);

const IntegrationRequestSchema = z
	.strictObject({
		operation: OperationSchema,
		adapterSource: NodePathSchema.nullish(),
		budgetMilliseconds: z.number().int().min(1).max(MAXIMUM_BUDGET_MILLISECONDS).nullish(),
	})
	.readonly();

const SnapshotItemSchema = z
	.strictObject({
		availability: z.enum(['available', 'not-found', 'unknown']),
		integrationId: ProviderSchema,
		issue: z
			.enum([
				'inspection-failed',
				'manual-action-required',
				'unsupported-platform',
				'unsupported-version',
			])
			.nullish(),
		managementMode: z.enum(['managed-local', 'manual-only']),
		state: z.enum([
			'conflict',
			'disabled',
			'enabled',
			'not-installed',
			'update-required',
			'unknown',
		]),
		timedOut: z.boolean(),
	})
	.readonly();

const CapsuleStatusSchema = z
	.strictObject({
		ready: z.boolean(),
		root: NodePathSchema,
		sourceKind: z.enum(['cohort-sidecar', 'adapter-source-override', 'absent']),
		bindingFailure: z.boolean(),
		payloadSha256: z.string(),
	})
	.readonly();

const MarketplaceSchema = z.enum(['absent', 'current', 'update-required']);
const TargetStateSchema = z.enum(['conflict', 'current', 'missing', 'unknown', 'update-required']);

const InspectionDetailSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('cli'),
			marketplace: MarketplaceSchema.nullish(),
			pluginPresent: z.boolean().nullish(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('codex'),
			marketplace: MarketplaceSchema,
			pluginPresent: z.boolean(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('guarded'),
			targetState: TargetStateSchema,
			sourceReady: z.boolean(),
			obsoleteTargetState: TargetStateSchema.nullish(),
		})
		.readonly(),
	z.strictObject({ kind: z.literal('plain') }).readonly(),
]);

const CommandStepSchema = z
	.strictObject({
		kind: z.literal('command'),
		executable: z.string(),
		argv: z.array(z.string()).readonly(),
	})
	.readonly();

const PlannedStepSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('capsule-publish'), target: NodePathSchema }).readonly(),
	CommandStepSchema,
	z.strictObject({ kind: z.literal('guarded-write'), target: NodePathSchema }).readonly(),
	z.strictObject({ kind: z.literal('guarded-remove'), target: NodePathSchema }).readonly(),
]);

const AttemptedStepSchema = z
	.strictObject({
		step: z.discriminatedUnion('kind', [
			CommandStepSchema,
			z
				.strictObject({
					kind: z.literal('filesystem'),
					operation: z.enum([
						'publish-capsule',
						'digest-verify',
						'move-aside',
						'publish-entry',
						'restore-backup',
						'remove-backup',
						'remove-entry',
						'prepare-codex-home',
						'stage-configuration',
						'move-configuration-aside',
						'preserve-configuration-metadata',
						'publish-configuration',
						'verify-configuration',
						'remove-configuration-backup',
						'restore-configuration-backup',
					]),
					path: NodePathSchema,
				})
				.readonly(),
		]),
		outcome: z.enum([
			'completed',
			'conflict',
			'failed',
			'deadline',
			'timed-out',
			'leaked',
			'output-too-large',
			'cancelled',
		]),
	})
	.readonly();

const VerdictSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('completed') }).readonly(),
	z
		.strictObject({
			kind: z.literal('refused'),
			code: z.enum([
				'agent-unavailable',
				'conflict',
				'manual-action-required',
				'inspection-failed',
				'capsule-unavailable',
				'timed-out',
			]),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('failed'),
			code: z.enum([
				'conflict',
				'manual-action-required',
				'operation-failed',
				'verification-failed',
				'timed-out',
				'output-too-large',
				'leaked',
			]),
		})
		.readonly(),
	z.strictObject({ kind: z.literal('unknown') }).readonly(),
	z.strictObject({ kind: z.literal('cancelled') }).readonly(),
]);

const MutationReportSchema = z
	.strictObject({
		integrationId: ProviderSchema,
		direction: z.enum(['enable', 'disable']),
		attempted: z.array(AttemptedStepSchema).readonly(),
		residuals: z.array(NodePathSchema).readonly(),
		observed: SnapshotItemSchema.nullish(),
		provenance: z.enum(['pre-effect', 'post-effect', 'partial', 'unavailable']),
		effectBegun: z.boolean(),
		verdict: VerdictSchema,
	})
	.readonly();

const IntegrationReportSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('list'),
			items: z.array(SnapshotItemSchema).readonly(),
			capsule: CapsuleStatusSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('explain'),
			item: SnapshotItemSchema,
			detail: InspectionDetailSchema,
			capsule: CapsuleStatusSchema,
			plan: z.array(PlannedStepSchema).readonly(),
		})
		.readonly(),
	z.strictObject({ kind: z.literal('mutation'), report: MutationReportSchema }).readonly(),
	z.strictObject({ kind: z.literal('home-unavailable') }).readonly(),
]);

/** The Worker generation and job a poll or cancel is fenced to. */
const JobSchema = z
	.strictObject({
		workerId: WorkerIdSchema,
		generation: WorkbenchRevisionSchema.refine((value) => value !== '0'),
		jobId: z.string().regex(/^[0-9a-f]{32}$/u),
	})
	.readonly();

const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	sessionId: SessionIdSchema,
};

export const AgentIntegrationRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.agent-integration'),
		...correlation,
		action: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('start'), request: IntegrationRequestSchema }).readonly(),
			z.strictObject({ kind: z.literal('poll'), job: JobSchema }).readonly(),
			z.strictObject({ kind: z.literal('cancel'), job: JobSchema }).readonly(),
		]),
	})
	.readonly();

export const AgentIntegrationResultSchema = z
	.strictObject({
		kind: z.literal('workbench.agent-integration.result'),
		...correlation,
		outcome: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('accepted'), job: JobSchema }).readonly(),
			z.strictObject({ kind: z.literal('pending'), job: JobSchema }).readonly(),
			z
				.strictObject({
					kind: z.literal('completed'),
					job: JobSchema,
					report: IntegrationReportSchema,
				})
				.readonly(),
			z.strictObject({ kind: z.literal('cancel-registered'), job: JobSchema }).readonly(),
			z.strictObject({ kind: z.literal('already-terminal'), job: JobSchema }).readonly(),
			z
				.strictObject({
					kind: z.literal('failed'),
					reason: z.enum([
						'denied',
						'session-mismatch',
						'invalid-request',
						'worker-unavailable',
						'worker-changed',
						'busy',
						'job-unknown',
						'abandoned',
						'malformed',
					]),
				})
				.readonly(),
		]),
	})
	.readonly();
