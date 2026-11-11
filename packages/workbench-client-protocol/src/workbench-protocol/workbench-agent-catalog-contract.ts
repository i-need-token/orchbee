import { z } from 'zod';
import {
	SessionIdSchema,
	WorkbenchAgentStartProviderSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';

/** Thirty-six rows of display text and the Recent list, with room for the catalog to grow. */
export const WORKBENCH_MAXIMUM_ENCODED_AGENT_CATALOG_BYTES = 32 * 1024;
const utf8 = new TextEncoder();
const STARTABLE_PROVIDERS = WorkbenchAgentStartProviderSchema.options;

const AgentCatalogEntrySchema = z
	.strictObject({
		providerId: WorkbenchAgentStartProviderSchema,
		/** The provider's product name, never translated. */
		productName: z
			.string()
			.min(1)
			.refine((name) => utf8.encode(name).byteLength <= 64 && !/\p{Cc}/u.test(name)),
		/** The command and its fixed arguments joined by one space; display only. */
		command: z.string().regex(/^[ -~]{1,128}$/u),
		state: z.enum(['found', 'not-found', 'cant-check', 'checking', 'unknown']),
	})
	.readonly();

const WorkbenchAgentCatalogSchema = z
	.strictObject({
		/** Starts at 1 and advances on any change of the other members. */
		revision: WorkbenchRevisionSchema.refine((revision) => revision !== '0'),
		/** Whether this Session's machine can start Agents at all. */
		startable: z.boolean(),
		lookup: z
			.strictObject({
				status: z.enum(['running', 'complete', 'failed', 'unsupported']),
				/** Unix milliseconds of the last completed lookup; null while none has completed. */
				checkedAt: z.number().int().min(1).max(Number.MAX_SAFE_INTEGER).nullable(),
			})
			.readonly(),
		/** The providers started most recently, newest first, one each. */
		recent: z
			.array(WorkbenchAgentStartProviderSchema)
			.max(8)
			.refine((recent) => new Set(recent).size === recent.length)
			.readonly(),
		/** One row per startable provider, in catalog order. */
		agents: z
			.array(AgentCatalogEntrySchema)
			.max(64)
			.refine(
				(agents) =>
					agents.length === STARTABLE_PROVIDERS.length &&
					agents.every((row, index) => row.providerId === STARTABLE_PROVIDERS[index]),
			)
			.readonly(),
	})
	.readonly();

const identity = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	sessionId: SessionIdSchema,
};
export const AgentCatalogRequestSchema = z
	.strictObject({
		...identity,
		kind: z.literal('workbench.agent-catalog'),
		/** `refresh` also asks for a new lookup; both answer with the catalog as it stands. */
		action: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('current') }).readonly(),
			z.strictObject({ kind: z.literal('refresh') }).readonly(),
		]),
	})
	.readonly();
export const AgentCatalogResultSchema = z
	.strictObject({
		...identity,
		kind: z.literal('workbench.agent-catalog.result'),
		outcome: z.discriminatedUnion('kind', [
			z
				.strictObject({ kind: z.literal('current'), catalog: WorkbenchAgentCatalogSchema })
				.readonly(),
			z
				.strictObject({ kind: z.literal('failed'), reason: z.enum(['denied', 'unavailable']) })
				.readonly(),
		]),
	})
	.readonly();
