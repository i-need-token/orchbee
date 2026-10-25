import { z } from 'zod';
import {
	SiteIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchWorkspaceIdSchema,
	WorkbenchRevisionSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';
const GitDivergenceSchema = z
	.strictObject({
		ahead: z.number().int().min(0).max(4294967295),
		behind: z.number().int().min(0).max(4294967295),
	})
	.readonly();
const identity = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	siteId: SiteIdSchema,
	workspaceId: WorkbenchWorkspaceIdSchema,
};
export const WorkspaceGitRequestSchema = z
	.strictObject({ ...identity, kind: z.literal('workbench.workspace-git') })
	.readonly();
const WorkspaceGitOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('observed'),
			workbenchRevision: WorkbenchRevisionSchema,
			branch: z
				.string()
				.min(1)
				.refine(
					(v) => new TextEncoder().encode(v).length <= 1024 && !/[\p{Cc}\uD800-\uDFFF]/u.test(v),
				)
				.nullable(),
			divergence: GitDivergenceSchema.nullable(),
		})
		.refine((v) => v.branch !== null || v.divergence === null)
		.readonly(),
	z
		.strictObject({
			kind: z.literal('failed'),
			reason: z.enum([
				'denied',
				'unavailable',
				'not-found',
				'source-changed',
				'timeout',
				'malformed',
			]),
		})
		.readonly(),
]);
export const WorkspaceGitResultSchema = z
	.strictObject({
		...identity,
		kind: z.literal('workbench.workspace-git.result'),
		outcome: WorkspaceGitOutcomeSchema,
	})
	.readonly();
