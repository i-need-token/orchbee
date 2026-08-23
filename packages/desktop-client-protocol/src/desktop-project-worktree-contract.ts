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
		projectId: identifierSchema,
		requestId: DesktopProjectWorktreeRequestIdSchema,
		type: z.literal('desktop-project-worktree-discovery-requested'),
	}),
	z.strictObject({
		baseRef: gitValueSchema,
		branchName: gitValueSchema,
		createWorkspace: z.boolean(),
		projectId: identifierSchema,
		repositoryId: identifierSchema,
		requestId: DesktopProjectWorktreeRequestIdSchema,
		sourceDirectoryId: identifierSchema,
		type: z.literal('desktop-project-worktree-creation-requested'),
	}),
]);

export const DesktopProjectWorktreeFailureCodeSchema = z.enum([
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
]);

export const DesktopProjectWorktreeRepositorySchema = z.strictObject({
	defaultBaseRef: gitValueSchema,
	label: z.string().min(1).max(512),
	localBaseRefs: z.array(gitValueSchema).max(10_000),
	repositoryId: identifierSchema,
	sourceDirectoryId: identifierSchema,
	suggestedBranchName: gitValueSchema,
});

export const DesktopProjectWorktreeDiscoverySchema = z.strictObject({
	managedRoot: managedPathSchema,
	repositories: z.array(DesktopProjectWorktreeRepositorySchema).max(256),
});

export const DesktopProjectWorktreeCreationResultSchema = z.strictObject({
	directoryId: identifierSchema,
	workspaceId: identifierSchema.nullable(),
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
		code: DesktopProjectWorktreeFailureCodeSchema,
		operation: z.enum(['creation', 'discovery']),
		requestId: DesktopProjectWorktreeRequestIdSchema,
		type: z.literal('desktop-project-worktree-failed'),
	}),
]);

export type DesktopProjectWorktreeCreationResult = z.infer<
	typeof DesktopProjectWorktreeCreationResultSchema
>;
export type DesktopProjectWorktreeDiscovery = z.infer<typeof DesktopProjectWorktreeDiscoverySchema>;
export type DesktopProjectWorktreeFailureCode = z.infer<
	typeof DesktopProjectWorktreeFailureCodeSchema
>;
export type DesktopProjectWorktreeMainEvent = z.infer<typeof DesktopProjectWorktreeMainEventSchema>;
export type DesktopProjectWorktreeRendererCommand = z.infer<
	typeof DesktopProjectWorktreeRendererCommandSchema
>;
export type DesktopProjectWorktreeRequestId = z.infer<typeof DesktopProjectWorktreeRequestIdSchema>;
