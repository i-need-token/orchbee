import { z } from 'zod';

const MAXIMUM_NAME_CHARACTERS = 256;
const MAXIMUM_DESTINATION_CHARACTERS = 1_024;
const MAXIMUM_PATH_CHARACTERS = 32_768;
const MAXIMUM_PROMPT_CHARACTERS = 4_096;
const MAXIMUM_RESPONSE_CHARACTERS = 4_096;

export const DesktopSshProjectEnrollmentRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopSshProjectEnrollmentRequestId'>();

const WorkbenchProjectIdSchema = z.string().min(1).max(MAXIMUM_PATH_CHARACTERS);

const EnvironmentNameSchema = z
	.string()
	.min(1)
	.max(MAXIMUM_NAME_CHARACTERS)
	.refine((value) => value === value.trim())
	.refine(hasNoControlCharacters);

const SshDestinationSchema = z
	.string()
	.min(1)
	.max(MAXIMUM_DESTINATION_CHARACTERS)
	.refine((value) => value === value.trim())
	.refine(hasNoControlCharacters);

const AbsolutePosixPathSchema = z
	.string()
	.min(1)
	.max(MAXIMUM_PATH_CHARACTERS)
	.refine((value) => value.startsWith('/') && !value.includes('\0'));

export const DesktopSshProjectEnrollmentInputSchema = z.strictObject({
	configurationFilePath: AbsolutePosixPathSchema.nullable(),
	destination: SshDestinationSchema,
	name: EnvironmentNameSchema,
	projectPath: AbsolutePosixPathSchema,
});

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
]);

export const DesktopSshProjectEnrollmentProgressStageSchema = z.enum([
	'resolving-target',
	'recording-environment',
	'connecting',
	'inspecting-project',
	'committing-project',
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
		result: DesktopSshProjectEnrollmentResultSchema,
		type: z.literal('desktop-ssh-project-enrollment-settled'),
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
]);

export type DesktopSshProjectConnectionResult = z.infer<
	typeof DesktopSshProjectConnectionResultSchema
>;
export type DesktopSshProjectConnectionStatus = z.infer<
	typeof DesktopSshProjectConnectionStatusSchema
>;
export type DesktopSshProjectEnrollmentInput = z.infer<
	typeof DesktopSshProjectEnrollmentInputSchema
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
