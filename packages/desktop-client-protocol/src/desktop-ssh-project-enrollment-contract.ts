import { z } from 'zod';

const MAXIMUM_DESTINATION_CHARACTERS = 1_024;
const MAXIMUM_DIRECTORY_ENTRIES = 256;
const MAXIMUM_DIRECTORY_NAME_CHARACTERS = 255;
const MAXIMUM_PATH_CHARACTERS = 32_768;
const MAXIMUM_PROMPT_CHARACTERS = 4_096;
const MAXIMUM_RESPONSE_CHARACTERS = 4_096;
const MAXIMUM_ENVIRONMENT_NAME_CHARACTERS = 256;
const MAXIMUM_SSH_ENVIRONMENT_COUNT = 1_024;

export const DesktopSshProjectEnrollmentRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopSshProjectEnrollmentRequestId'>();

const WorkbenchProjectIdSchema = z.string().min(1).max(MAXIMUM_PATH_CHARACTERS);
const WorkbenchDirectoryIdSchema = z.string().min(1).max(MAXIMUM_PATH_CHARACTERS);
export const DesktopSshEnvironmentIdSchema = z
	.string()
	.regex(/^environment:[0-9a-f]{32}$/u)
	.brand<'DesktopSshEnvironmentId'>();

const SshDestinationSchema = z
	.string()
	.min(1)
	.max(MAXIMUM_DESTINATION_CHARACTERS)
	.refine((value) => value === value.trim())
	.refine(hasNoControlCharacters);

const SshHostAliasSchema = z
	.string()
	.min(1)
	.max(MAXIMUM_DESTINATION_CHARACTERS)
	.refine((value) => value === value.trim())
	.refine(hasNoControlCharacters)
	.refine((value) => !value.startsWith('-') && !/\s|[!?*[\]]/u.test(value));

const AbsolutePosixPathSchema = z
	.string()
	.min(1)
	.max(MAXIMUM_PATH_CHARACTERS)
	.refine((value) => value.startsWith('/') && !value.includes('\0'));

export const DesktopSshProjectEnrollmentInputSchema = z.strictObject({
	destination: SshDestinationSchema,
});

export const DesktopSshProjectDirectorySelectionRevisionSchema = z
	.number()
	.int()
	.min(1)
	.max(Number.MAX_SAFE_INTEGER);

export const DesktopSshProjectDirectoryNavigationFailureReasonSchema = z.enum([
	'invalid-path',
	'not-found',
	'not-directory',
	'permission-denied',
	'timed-out',
	'busy',
	'unavailable',
]);

const DesktopSshProjectDirectoryNameSchema = z
	.string()
	.min(1)
	.max(MAXIMUM_DIRECTORY_NAME_CHARACTERS)
	.refine((value) => value !== '.' && value !== '..')
	.refine((value) => !value.includes('/'))
	.refine(hasNoControlCharacters);

export const DesktopSshProjectDirectorySnapshotSchema = z.strictObject({
	canonicalPath: AbsolutePosixPathSchema,
	directories: z
		.array(z.strictObject({ name: DesktopSshProjectDirectoryNameSchema }))
		.max(MAXIMUM_DIRECTORY_ENTRIES),
	navigationFailure: DesktopSshProjectDirectoryNavigationFailureReasonSchema.nullable(),
	parentPath: AbsolutePosixPathSchema.nullable(),
	truncated: z.boolean(),
});

export const DesktopSshProjectDirectorySelectionResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('cancelled') }),
	z.strictObject({ kind: z.literal('navigate'), path: AbsolutePosixPathSchema }),
	z.strictObject({ kind: z.literal('selected') }),
]);

export const DesktopSshProjectInteractionHandleSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopSshProjectInteractionHandle'>();

export const DesktopSshProjectInteractionResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('cancelled') }),
	z.strictObject({
		kind: z.literal('submitted'),
		response: z.string().max(MAXIMUM_RESPONSE_CHARACTERS).refine(hasNoResponseControlCharacters),
	}),
]);

export const DesktopSshProjectEnrollmentRendererCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		input: DesktopSshProjectEnrollmentInputSchema,
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		type: z.literal('desktop-ssh-project-enrollment-requested'),
	}),
	z.strictObject({
		input: DesktopSshProjectEnrollmentInputSchema,
		projectId: WorkbenchProjectIdSchema,
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		type: z.literal('desktop-ssh-project-folder-attachment-requested'),
	}),
	z.strictObject({
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		type: z.literal('desktop-ssh-project-enrollment-cancel-requested'),
	}),
	z.strictObject({
		generation: z.number().int().min(1).max(Number.MAX_SAFE_INTEGER),
		handle: DesktopSshProjectInteractionHandleSchema,
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		result: DesktopSshProjectInteractionResultSchema,
		type: z.literal('desktop-ssh-project-interaction-resolved'),
	}),
	z.strictObject({
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		result: DesktopSshProjectDirectorySelectionResultSchema,
		revision: DesktopSshProjectDirectorySelectionRevisionSchema,
		type: z.literal('desktop-ssh-project-directory-selection-resolved'),
	}),
	z.strictObject({
		projectId: WorkbenchProjectIdSchema,
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		type: z.literal('desktop-ssh-project-connect-requested'),
	}),
	z.strictObject({
		projectId: WorkbenchProjectIdSchema,
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		type: z.literal('desktop-ssh-project-disconnect-requested'),
	}),
	z.strictObject({
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		type: z.literal('desktop-ssh-project-status-requested'),
	}),
	z.strictObject({
		refreshDiscovery: z.boolean(),
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		type: z.literal('desktop-ssh-environment-catalog-requested'),
	}),
	z.strictObject({
		input: DesktopSshProjectEnrollmentInputSchema,
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		type: z.literal('desktop-ssh-environment-add-requested'),
	}),
	z.strictObject({
		environmentId: DesktopSshEnvironmentIdSchema,
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		type: z.literal('desktop-ssh-environment-connect-requested'),
	}),
	z.strictObject({
		environmentId: DesktopSshEnvironmentIdSchema,
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		type: z.literal('desktop-ssh-environment-disconnect-requested'),
	}),
]);

export const DesktopSshProjectEnrollmentProgressStageSchema = z.enum([
	'resolving-target',
	'recording-environment',
	'connecting',
	'selecting-project-directory',
	'inspecting-project',
	'committing-project',
	'committing-project-folder',
]);

export const DesktopSshProjectConnectionBlockedReasonSchema = z.enum([
	'authentication-cancelled',
	'authentication-required',
	'compatibility-unsupported',
	'configuration-invalid',
	'connection-failed',
	'host-verification-failed',
	'policy-rejected',
	'runtime-release-unavailable',
]);

export const DesktopSshProjectConnectionStatusSchema = z.strictObject({
	blockedReason: DesktopSshProjectConnectionBlockedReasonSchema.nullable(),
	projectId: WorkbenchProjectIdSchema,
	state: z.enum([
		'blocked',
		'connected',
		'connecting',
		'disconnected',
		'interaction-required',
		'retrying',
		'unavailable',
	]),
});

export const DesktopSshEnvironmentDiscoveryIssueSchema = z.enum([
	'invalid-config',
	'read-failed',
	'truncated',
]);

export const DesktopSshEnvironmentConnectionStateSchema = z.enum([
	'blocked',
	'connected',
	'connecting',
	'disconnected',
	'interaction-required',
	'retrying',
	'unavailable',
]);

export const DesktopSshEnvironmentCatalogSchema = z.strictObject({
	discoveryIssue: DesktopSshEnvironmentDiscoveryIssueSchema.nullable(),
	managed: z
		.array(
			z.strictObject({
				blockedReason: DesktopSshProjectConnectionBlockedReasonSchema.nullable(),
				destination: SshDestinationSchema,
				environmentId: DesktopSshEnvironmentIdSchema,
				name: z.string().min(1).max(MAXIMUM_ENVIRONMENT_NAME_CHARACTERS),
				projectCount: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER),
				state: DesktopSshEnvironmentConnectionStateSchema,
			}),
		)
		.max(MAXIMUM_SSH_ENVIRONMENT_COUNT),
	registryIssue: z.literal('unavailable').nullable(),
	suggestions: z
		.array(z.strictObject({ destination: SshHostAliasSchema }))
		.max(MAXIMUM_SSH_ENVIRONMENT_COUNT),
});

export const DesktopSshEnvironmentAddResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		environmentId: DesktopSshEnvironmentIdSchema,
		kind: z.enum(['added', 'existing']),
	}),
	z.strictObject({ kind: z.literal('cancelled') }),
	z.strictObject({
		kind: z.literal('failed'),
		reason: z.enum([
			'busy',
			'environment-capacity',
			'environment-conflict',
			'registry-unavailable',
			'target-resolution-failed',
		]),
	}),
	z.strictObject({
		kind: z.literal('indeterminate'),
		reason: z.enum(['control-unavailable', 'durability-unconfirmed']),
	}),
]);

export const DesktopSshEnvironmentConnectionResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		environmentId: DesktopSshEnvironmentIdSchema,
		kind: z.enum(['connected', 'disconnected']),
	}),
	z.strictObject({ kind: z.literal('cancelled') }),
	z.strictObject({
		kind: z.literal('failed'),
		reason: z.enum([
			'busy',
			'connection-owner-unavailable',
			'environment-not-found',
			'local-platform-unsupported',
			'registry-unavailable',
			'remote-runtime-unavailable',
		]),
	}),
	z.strictObject({
		kind: z.literal('indeterminate'),
		reason: z.literal('control-unavailable'),
	}),
]);

export const DesktopSshProjectEnrollmentFailureReasonSchema = z.enum([
	'busy',
	'connection-owner-unavailable',
	'environment-capacity',
	'environment-conflict',
	'environment-not-found',
	'local-platform-unsupported',
	'project-root-invalid',
	'project-root-not-found',
	'project-root-not-directory',
	'project-root-permission-denied',
	'project-root-timed-out',
	'project-root-busy',
	'registry-unavailable',
	'remote-runtime-unavailable',
	'target-resolution-failed',
	'workbench-missing',
	'workbench-unavailable',
]);

export const DesktopSshProjectEnrollmentResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		canonicalPath: AbsolutePosixPathSchema,
		kind: z.enum(['created', 'existing']),
		projectId: WorkbenchProjectIdSchema,
		workspaceId: WorkbenchProjectIdSchema,
	}),
	z.strictObject({ kind: z.literal('cancelled') }),
	z.strictObject({
		kind: z.literal('failed'),
		reason: DesktopSshProjectEnrollmentFailureReasonSchema,
	}),
	z.strictObject({
		kind: z.literal('indeterminate'),
		reason: z.enum(['control-unavailable', 'durability-unconfirmed']),
	}),
]);

export const DesktopSshProjectFolderAttachmentFailureReasonSchema = z.enum([
	'busy',
	'connection-owner-unavailable',
	'environment-capacity',
	'environment-conflict',
	'environment-not-found',
	'local-platform-unsupported',
	'project-environment-conflict',
	'project-folder-already-owned',
	'project-not-found',
	'project-root-invalid',
	'project-root-not-found',
	'project-root-not-directory',
	'project-root-permission-denied',
	'project-root-timed-out',
	'project-root-busy',
	'registry-unavailable',
	'remote-runtime-unavailable',
	'target-resolution-failed',
	'workbench-missing',
	'workbench-unavailable',
]);

export const DesktopSshProjectFolderAttachmentResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		canonicalPath: AbsolutePosixPathSchema,
		directoryId: WorkbenchDirectoryIdSchema,
		kind: z.enum(['attached', 'existing']),
		projectId: WorkbenchProjectIdSchema,
	}),
	z.strictObject({ kind: z.literal('cancelled') }),
	z.strictObject({
		kind: z.literal('failed'),
		reason: DesktopSshProjectFolderAttachmentFailureReasonSchema,
	}),
	z.strictObject({
		kind: z.literal('indeterminate'),
		reason: z.enum(['control-unavailable', 'durability-unconfirmed']),
	}),
]);

export const DesktopSshProjectConnectionResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.enum(['connected', 'disconnected']),
		projectId: WorkbenchProjectIdSchema,
	}),
	z.strictObject({ kind: z.literal('cancelled') }),
	z.strictObject({
		kind: z.literal('failed'),
		reason: z.enum([
			'busy',
			'connection-owner-unavailable',
			'environment-not-found',
			'local-platform-unsupported',
			'project-not-found',
			'registry-unavailable',
			'remote-runtime-unavailable',
			'workbench-missing',
			'workbench-unavailable',
		]),
	}),
	z.strictObject({
		kind: z.literal('indeterminate'),
		reason: z.literal('control-unavailable'),
	}),
]);

export const DesktopSshProjectEnrollmentMainEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		stage: DesktopSshProjectEnrollmentProgressStageSchema,
		type: z.literal('desktop-ssh-project-enrollment-progressed'),
	}),
	z.strictObject({
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		stage: DesktopSshProjectEnrollmentProgressStageSchema,
		type: z.literal('desktop-ssh-project-folder-attachment-progressed'),
	}),
	z.strictObject({
		generation: z.number().int().min(1).max(Number.MAX_SAFE_INTEGER),
		handle: DesktopSshProjectInteractionHandleSchema,
		kind: z.enum(['confirm', 'none', 'unknown']),
		prompt: z
			.string()
			.max(MAXIMUM_PROMPT_CHARACTERS)
			.refine((value) => !value.includes('\0')),
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		type: z.literal('desktop-ssh-project-interaction-requested'),
	}),
	z.strictObject({
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		revision: DesktopSshProjectDirectorySelectionRevisionSchema,
		snapshot: DesktopSshProjectDirectorySnapshotSchema,
		type: z.literal('desktop-ssh-project-directory-selection-requested'),
	}),
	z.strictObject({
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		result: DesktopSshProjectEnrollmentResultSchema,
		type: z.literal('desktop-ssh-project-enrollment-settled'),
	}),
	z.strictObject({
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		result: DesktopSshProjectFolderAttachmentResultSchema,
		type: z.literal('desktop-ssh-project-folder-attachment-settled'),
	}),
	z.strictObject({
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		result: DesktopSshProjectConnectionResultSchema,
		type: z.literal('desktop-ssh-project-connection-settled'),
	}),
	z.strictObject({
		requestId: DesktopSshProjectEnrollmentRequestIdSchema.nullable(),
		statuses: z.array(DesktopSshProjectConnectionStatusSchema).max(1_024),
		type: z.literal('desktop-ssh-project-status-published'),
	}),
	z.strictObject({
		catalog: DesktopSshEnvironmentCatalogSchema,
		requestId: DesktopSshProjectEnrollmentRequestIdSchema.nullable(),
		type: z.literal('desktop-ssh-environment-catalog-published'),
	}),
	z.strictObject({
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		result: DesktopSshEnvironmentAddResultSchema,
		type: z.literal('desktop-ssh-environment-add-settled'),
	}),
	z.strictObject({
		requestId: DesktopSshProjectEnrollmentRequestIdSchema,
		result: DesktopSshEnvironmentConnectionResultSchema,
		type: z.literal('desktop-ssh-environment-connection-settled'),
	}),
]);

export type DesktopSshEnvironmentAddResult = z.infer<typeof DesktopSshEnvironmentAddResultSchema>;
export type DesktopSshEnvironmentCatalog = z.infer<typeof DesktopSshEnvironmentCatalogSchema>;
export type DesktopSshEnvironmentConnectionResult = z.infer<
	typeof DesktopSshEnvironmentConnectionResultSchema
>;
export type DesktopSshEnvironmentConnectionState = z.infer<
	typeof DesktopSshEnvironmentConnectionStateSchema
>;
export type DesktopSshEnvironmentDiscoveryIssue = z.infer<
	typeof DesktopSshEnvironmentDiscoveryIssueSchema
>;
export type DesktopSshEnvironmentId = z.infer<typeof DesktopSshEnvironmentIdSchema>;

export type DesktopSshProjectConnectionResult = z.infer<
	typeof DesktopSshProjectConnectionResultSchema
>;
export type DesktopSshProjectConnectionStatus = z.infer<
	typeof DesktopSshProjectConnectionStatusSchema
>;
export type DesktopSshProjectEnrollmentInput = z.infer<
	typeof DesktopSshProjectEnrollmentInputSchema
>;
export type DesktopSshProjectDirectoryNavigationFailureReason = z.infer<
	typeof DesktopSshProjectDirectoryNavigationFailureReasonSchema
>;
export type DesktopSshProjectDirectorySelectionResult = z.infer<
	typeof DesktopSshProjectDirectorySelectionResultSchema
>;
export type DesktopSshProjectDirectorySnapshot = z.infer<
	typeof DesktopSshProjectDirectorySnapshotSchema
>;
export type DesktopSshProjectEnrollmentFailureReason = z.infer<
	typeof DesktopSshProjectEnrollmentFailureReasonSchema
>;
export type DesktopSshProjectEnrollmentProgressStage = z.infer<
	typeof DesktopSshProjectEnrollmentProgressStageSchema
>;
export type DesktopSshProjectEnrollmentMainEvent = z.infer<
	typeof DesktopSshProjectEnrollmentMainEventSchema
>;
export type DesktopSshProjectEnrollmentRendererCommand = z.infer<
	typeof DesktopSshProjectEnrollmentRendererCommandSchema
>;
export type DesktopSshProjectEnrollmentRequestId = z.infer<
	typeof DesktopSshProjectEnrollmentRequestIdSchema
>;
export type DesktopSshProjectEnrollmentResult = z.infer<
	typeof DesktopSshProjectEnrollmentResultSchema
>;
export type DesktopSshProjectFolderAttachmentFailureReason = z.infer<
	typeof DesktopSshProjectFolderAttachmentFailureReasonSchema
>;
export type DesktopSshProjectFolderAttachmentResult = z.infer<
	typeof DesktopSshProjectFolderAttachmentResultSchema
>;
export type DesktopSshProjectInteractionResult = z.infer<
	typeof DesktopSshProjectInteractionResultSchema
>;

function hasNoControlCharacters(value: string): boolean {
	for (const character of value) {
		const codePoint = character.codePointAt(0);
		if (codePoint !== undefined && (codePoint <= 0x1f || codePoint === 0x7f)) return false;
	}
	return true;
}

function hasNoResponseControlCharacters(value: string): boolean {
	return !value.includes('\0') && !value.includes('\r') && !value.includes('\n');
}
