import { z } from 'zod';

const SafeIdentifierSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u);
const ConcreteDestinationSchema = z
	.string()
	.min(1)
	.max(1_024)
	.regex(/^(?!-)\S+$/u)
	.refine(hasNoControlCharacters);
const SshNameSchema = z.string().min(1).max(1_024).regex(/^\S+$/u).refine(hasNoControlCharacters);

export const SshEnvironmentConnectionIdSchema =
	SafeIdentifierSchema.brand<'SshEnvironmentConnectionId'>();

export const SshEnvironmentInteractionHandleSchema =
	SafeIdentifierSchema.brand<'SshEnvironmentInteractionHandle'>();

const SshEnvironmentDestinationSchema =
	ConcreteDestinationSchema.brand<'SshEnvironmentDestination'>();

const SshEnvironmentConfigurationFilePathSchema = z
	.string()
	.min(1)
	.max(32_768)
	.refine(hasNoControlCharacters)
	.brand<'SshEnvironmentConfigurationFilePath'>();

export const SshEnvironmentTargetReferenceSchema = z.strictObject({
	configurationFilePath: SshEnvironmentConfigurationFilePathSchema.nullable(),
	destination: SshEnvironmentDestinationSchema,
});

const SshEnvironmentHostKeyPolicySchema = z.enum(['yes', 'ask', 'accept-new']);

export const SshEnvironmentResolvedTargetSchema = z.strictObject({
	destination: SshEnvironmentDestinationSchema,
	hostKeyLookupName: SshNameSchema,
	hostKeyPolicy: SshEnvironmentHostKeyPolicySchema,
	hostname: SshNameSchema,
	port: z.number().int().min(1).max(65_535),
	user: SshNameSchema,
});

export type SshEnvironmentConnectionId = z.infer<typeof SshEnvironmentConnectionIdSchema>;
export type SshEnvironmentHostKeyPolicy = z.infer<typeof SshEnvironmentHostKeyPolicySchema>;
export type SshEnvironmentInteractionHandle = z.infer<typeof SshEnvironmentInteractionHandleSchema>;
export type SshEnvironmentResolvedTarget = z.infer<typeof SshEnvironmentResolvedTargetSchema>;
export type SshEnvironmentTargetReference = z.infer<typeof SshEnvironmentTargetReferenceSchema>;

function hasNoControlCharacters(value: string): boolean {
	for (const character of value) {
		const codePoint = character.codePointAt(0);
		if (codePoint !== undefined && (codePoint <= 0x1f || codePoint === 0x7f)) return false;
	}
	return true;
}
