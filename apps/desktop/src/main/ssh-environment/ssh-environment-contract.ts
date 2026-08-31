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

const SshEnvironmentOptionValueSchema = z
	.string()
	.min(1)
	.max(32_768)
	.refine(hasNoControlCharacters);

const SshEnvironmentTargetOptionSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('address-family'), value: z.enum(['inet', 'inet6']) }),
	z.strictObject({ enabled: z.boolean(), kind: z.literal('compression') }),
	z.strictObject({ enabled: z.boolean(), kind: z.literal('forward-agent') }),
	z.strictObject({ kind: z.literal('identity-file'), value: SshEnvironmentOptionValueSchema }),
	z.strictObject({
		kind: z.literal('proxy-jump'),
		value: ConcreteDestinationSchema,
	}),
	z.strictObject({ kind: z.literal('user'), value: SshNameSchema }),
	z.strictObject({ kind: z.literal('port'), value: z.number().int().min(1).max(65_535) }),
]);

export type SshEnvironmentTargetOption = z.infer<typeof SshEnvironmentTargetOptionSchema>;

interface CanonicalSshEnvironmentTargetReference {
	readonly configurationFilePath: null | z.infer<typeof SshEnvironmentConfigurationFilePathSchema>;
	readonly destination: z.infer<typeof SshEnvironmentDestinationSchema>;
	readonly options?: SshEnvironmentTargetOption[] | undefined;
}

export const SshEnvironmentTargetReferenceSchema = z
	.strictObject({
		configurationFilePath: SshEnvironmentConfigurationFilePathSchema.nullable(),
		destination: SshEnvironmentDestinationSchema,
		options: z.array(SshEnvironmentTargetOptionSchema).max(32).optional(),
	})
	.superRefine((target, context) => {
		const singletonKinds = new Set<string>();
		for (const [index, option] of (target.options ?? []).entries()) {
			if (option.kind === 'identity-file') continue;
			if (singletonKinds.has(option.kind)) {
				context.addIssue({
					code: 'custom',
					message: 'A singleton SSH target option cannot be repeated.',
					path: ['options', index],
				});
			}
			singletonKinds.add(option.kind);
		}
	})
	.transform((target): CanonicalSshEnvironmentTargetReference => {
		const options = target.options
			?.slice()
			.sort((left, right) => sshTargetOptionOrder(left.kind) - sshTargetOptionOrder(right.kind));
		return options === undefined || options.length === 0
			? {
					configurationFilePath: target.configurationFilePath,
					destination: target.destination,
				}
			: { ...target, options };
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

function sshTargetOptionOrder(kind: SshEnvironmentTargetOption['kind']): number {
	switch (kind) {
		case 'address-family':
			return 0;
		case 'forward-agent':
			return 1;
		case 'compression':
			return 2;
		case 'identity-file':
			return 3;
		case 'proxy-jump':
			return 4;
		case 'user':
			return 5;
		case 'port':
			return 6;
	}
}
