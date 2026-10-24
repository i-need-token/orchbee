import { z } from 'zod';

export const DesktopProjectWorktreeRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopProjectWorktreeRequestId'>();

const identifierSchema = z.string().min(1).max(256);
const gitValueSchema = z
	.string()
	.min(1)
	.max(1_024)
	.refine(
		(value) => !value.includes('\0') && !value.includes('\r') && !value.includes('\n'),
		'Git values cannot contain control separators.',
	);
const managedPathSchema = z
	.string()
	.min(1)
	.max(32_768)
	.refine((value) => !value.includes('\0'), 'Managed paths cannot contain NUL bytes.');

export const DesktopProjectWorktreeRendererCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		preferredDirectoryId: identifierSchema.nullable(),
		sourceWorkspaceId: identifierSchema,
		requestId: DesktopProjectWorktreeRequestIdSchema,
		type: z.literal('desktop-project-worktree-discovery-requested'),
	}),
	z.strictObject({
		baseRef: gitValueSchema,
		branchName: gitValueSchema,
		createWorkspace: z.boolean(),
		sourceWorkspaceId: identifierSchema,
		repositoryId: identifierSchema,
		requestId: DesktopProjectWorktreeRequestIdSchema,
		sourceDirectoryId: identifierSchema,
		type: z.literal('desktop-project-worktree-creation-requested'),
	}),
	z.strictObject({
		sourceWorkspaceId: identifierSchema,
		repositoryId: identifierSchema,
		requestId: DesktopProjectWorktreeRequestIdSchema,
		sourceDirectoryId: identifierSchema,
		type: z.literal('desktop-project-worktree-adoption-requested'),
		worktreeId: identifierSchema,
	}),
]);

const DesktopProjectWorktreeFailureCodeSchema = z.enum([
	'base-not-found',
	'branch-already-exists',
	'busy',
	'compensation-failed',
	'control-unavailable',
	'creation-failed',
	'discovery-failed',
	'document-conflict',
	'git-unavailable',
	'invalid-branch',
	'project-not-found',
	'recovery-required',
	'repository-not-found',
	'source-unavailable',
	'target-conflict',
	'worktree-retained',
	'worktree-not-found',
]);

const DesktopProjectExistingWorktreeSchema = z.strictObject({
	label: z.string().min(1).max(512),
	worktreeId: identifierSchema,
});

const DesktopProjectWorktreeInventorySchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('available'),
		worktrees: z.array(DesktopProjectExistingWorktreeSchema).max(10_000),
	}),
	z.strictObject({ kind: z.literal('unavailable') }),
]);

const DesktopProjectWorktreeRepositorySchema = z.strictObject({
	defaultBaseRef: gitValueSchema,
	label: z.string().min(1).max(512),
	localBaseRefs: z.array(gitValueSchema).max(10_000),
	repositoryId: identifierSchema,
	sourceDirectoryId: identifierSchema,
	suggestedBranchName: gitValueSchema,
	worktreeInventory: DesktopProjectWorktreeInventorySchema,
});

const DesktopProjectWorktreeDiscoverySchema = z.strictObject({
	managedRoot: managedPathSchema,
	repositories: z.array(DesktopProjectWorktreeRepositorySchema).max(256),
});

const DesktopProjectWorktreeCreationResultSchema = z.strictObject({
	directoryId: identifierSchema,
	workspaceId: identifierSchema.nullable(),
});

const DesktopProjectWorktreeAdoptionResultSchema = z.strictObject({
	directoryId: identifierSchema,
	workspaceId: identifierSchema,
});

export const DesktopProjectWorktreeMainEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		discovery: DesktopProjectWorktreeDiscoverySchema,
		requestId: DesktopProjectWorktreeRequestIdSchema,
		type: z.literal('desktop-project-worktree-discovery-completed'),
	}),
	z.strictObject({
		requestId: DesktopProjectWorktreeRequestIdSchema,
		result: DesktopProjectWorktreeCreationResultSchema,
		type: z.literal('desktop-project-worktree-creation-completed'),
	}),
	z.strictObject({
		requestId: DesktopProjectWorktreeRequestIdSchema,
		result: DesktopProjectWorktreeAdoptionResultSchema,
		type: z.literal('desktop-project-worktree-adoption-completed'),
	}),
	z.strictObject({
		code: DesktopProjectWorktreeFailureCodeSchema,
		operation: z.enum(['adoption', 'creation', 'discovery']),
		requestId: DesktopProjectWorktreeRequestIdSchema,
		type: z.literal('desktop-project-worktree-failed'),
	}),
]);

export type DesktopProjectWorktreeCreationResult = z.infer<
	typeof DesktopProjectWorktreeCreationResultSchema
>;
export type DesktopProjectWorktreeAdoptionResult = z.infer<
	typeof DesktopProjectWorktreeAdoptionResultSchema
>;
export type DesktopProjectWorktreeDiscovery = z.infer<typeof DesktopProjectWorktreeDiscoverySchema>;
export type DesktopProjectWorktreeFailureCode = z.infer<
	typeof DesktopProjectWorktreeFailureCodeSchema
>;

export type DesktopProjectWorktreeRequestId = z.infer<typeof DesktopProjectWorktreeRequestIdSchema>;
