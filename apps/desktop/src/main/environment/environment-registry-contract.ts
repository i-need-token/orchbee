import { isAbsolute } from 'node:path';

import { SshEnvironmentRuntimeInstanceIdSchema } from '@orchbee/ssh-environment-protocol';
import { z } from 'zod';

import { SshEnvironmentTargetReferenceSchema } from '../ssh-environment';

export const DESKTOP_ENVIRONMENT_REGISTRY_LIMITS = Object.freeze({
	maximumDocumentBytes: 1024 * 1024,
	maximumEnvironmentCount: 1024,
	maximumNameLength: 256,
});

const EnvironmentNameSchema = z
	.string()
	.min(1)
	.max(DESKTOP_ENVIRONMENT_REGISTRY_LIMITS.maximumNameLength)
	.refine((value) => value === value.trim())
	.refine(hasNoControlCharacters);

export const EnvironmentIdSchema = z
	.string()
	.regex(/^environment:[0-9a-f]{32}$/u)
	.brand<'EnvironmentId'>();

const PersistedSshEnvironmentTargetReferenceSchema =
	SshEnvironmentTargetReferenceSchema.superRefine((target, context) => {
		if (target.configurationFilePath === null || isAbsolute(target.configurationFilePath)) return;
		context.addIssue({
			code: 'custom',
			message: 'A persisted SSH configuration path must be absolute.',
			path: ['configurationFilePath'],
		});
	});

const LegacyPersistedSshEnvironmentTargetReferenceSchema = z
	.strictObject({
		configurationFilePath: z.string().min(1).max(32_768).refine(hasNoControlCharacters).nullable(),
		destination: z
			.string()
			.min(1)
			.max(1_024)
			.regex(/^(?!-)\S+$/u)
			.refine(hasNoControlCharacters),
	})
	.superRefine((target, context) => {
		if (target.configurationFilePath === null || isAbsolute(target.configurationFilePath)) return;
		context.addIssue({
			code: 'custom',
			message: 'A persisted SSH configuration path must be absolute.',
			path: ['configurationFilePath'],
		});
	});

export const EnvironmentRecordSchema = z.strictObject({
	environmentId: EnvironmentIdSchema,
	name: EnvironmentNameSchema,
	provider: z.strictObject({
		kind: z.literal('ssh'),
		runtimeInstanceId: SshEnvironmentRuntimeInstanceIdSchema,
		target: PersistedSshEnvironmentTargetReferenceSchema,
	}),
});

const LegacyEnvironmentRecordSchema = z.strictObject({
	environmentId: EnvironmentIdSchema,
	name: EnvironmentNameSchema,
	provider: z.strictObject({
		kind: z.literal('ssh'),
		runtimeInstanceId: SshEnvironmentRuntimeInstanceIdSchema,
		target: LegacyPersistedSshEnvironmentTargetReferenceSchema,
	}),
});

const EnvironmentRegistryRevisionSchema = z.number().int().min(0).max(Number.MAX_SAFE_INTEGER);

const utf8Encoder = new TextEncoder();

export const EnvironmentRegistryDocumentSchema = z
	.strictObject({
		environments: z
			.array(EnvironmentRecordSchema)
			.max(DESKTOP_ENVIRONMENT_REGISTRY_LIMITS.maximumEnvironmentCount),
		revision: EnvironmentRegistryRevisionSchema,
		schemaVersion: z.literal(2),
	})
	.superRefine(validateEnvironmentRegistryDocument);

export const LegacyEnvironmentRegistryDocumentSchema = z
	.strictObject({
		environments: z
			.array(LegacyEnvironmentRecordSchema)
			.max(DESKTOP_ENVIRONMENT_REGISTRY_LIMITS.maximumEnvironmentCount),
		revision: EnvironmentRegistryRevisionSchema,
		schemaVersion: z.literal(1),
	})
	.superRefine(validateLegacyEnvironmentRegistryDocument);

export type EnvironmentId = z.infer<typeof EnvironmentIdSchema>;
export type EnvironmentRecord = z.infer<typeof EnvironmentRecordSchema>;
export type EnvironmentRegistryDocument = z.infer<typeof EnvironmentRegistryDocumentSchema>;

export function createEmptyEnvironmentRegistryDocument(): EnvironmentRegistryDocument {
	return EnvironmentRegistryDocumentSchema.parse({
		environments: [],
		revision: 0,
		schemaVersion: 2,
	});
}

export function migrateLegacyEnvironmentRegistryDocument(
	legacy: z.infer<typeof LegacyEnvironmentRegistryDocumentSchema>,
): EnvironmentRegistryDocument {
	const canonical = LegacyEnvironmentRegistryDocumentSchema.parse(legacy);
	return EnvironmentRegistryDocumentSchema.parse({
		environments: canonical.environments.map((environment) => ({
			environmentId: environment.environmentId,
			name: environment.name,
			provider: {
				kind: environment.provider.kind,
				runtimeInstanceId: environment.provider.runtimeInstanceId,
				target: {
					configurationFilePath: environment.provider.target.configurationFilePath,
					destination: environment.provider.target.destination,
				},
			},
		})),
		revision: canonical.revision,
		schemaVersion: 2,
	});
}

export function sshEnvironmentTargetKey(target: EnvironmentRecord['provider']['target']): string {
	const canonical = SshEnvironmentTargetReferenceSchema.parse(target);
	return JSON.stringify([
		canonical.configurationFilePath,
		canonical.destination,
		canonical.options ?? [],
	]);
}

function validateEnvironmentRegistryDocument(
	document: {
		readonly environments: readonly z.infer<typeof EnvironmentRecordSchema>[];
		readonly revision: number;
	},
	context: z.RefinementCtx,
): void {
	validateEnvironmentRegistryDocumentBase(
		document,
		(environment) => sshEnvironmentTargetKey(environment.provider.target),
		context,
	);
}

function validateLegacyEnvironmentRegistryDocument(
	document: {
		readonly environments: readonly z.infer<typeof LegacyEnvironmentRecordSchema>[];
		readonly revision: number;
	},
	context: z.RefinementCtx,
): void {
	validateEnvironmentRegistryDocumentBase(
		document,
		(environment) =>
			JSON.stringify([
				environment.provider.target.configurationFilePath,
				environment.provider.target.destination,
			]),
		context,
	);
}

function validateEnvironmentRegistryDocumentBase<
	Environment extends { readonly environmentId: string },
>(
	document: { readonly environments: readonly Environment[]; readonly revision: number },
	targetKey: (environment: Environment) => string,
	context: z.RefinementCtx,
): void {
	const environmentIds = new Set<string>();
	const targetKeys = new Set<string>();
	for (const [index, environment] of document.environments.entries()) {
		if (environmentIds.has(environment.environmentId)) {
			context.addIssue({
				code: 'custom',
				message: 'Environment identities must be unique.',
				path: ['environments', index, 'environmentId'],
			});
		}
		environmentIds.add(environment.environmentId);
		const key = targetKey(environment);
		if (targetKeys.has(key)) {
			context.addIssue({
				code: 'custom',
				message: 'Explicit SSH targets must be unique.',
				path: ['environments', index, 'provider', 'target'],
			});
		}
		targetKeys.add(key);
	}
	if (document.revision !== document.environments.length) {
		context.addIssue({
			code: 'custom',
			message: 'The append-only registry revision must equal its admitted record count.',
			path: ['revision'],
		});
	}
	if (
		utf8Encoder.encode(JSON.stringify(document)).byteLength >
		DESKTOP_ENVIRONMENT_REGISTRY_LIMITS.maximumDocumentBytes
	) {
		context.addIssue({
			code: 'custom',
			message: 'The Environment registry exceeds its storage boundary.',
			path: [],
		});
	}
}

function hasNoControlCharacters(value: string): boolean {
	for (const character of value) {
		const codePoint = character.codePointAt(0);
		if (codePoint !== undefined && (codePoint <= 0x1f || codePoint === 0x7f)) return false;
	}
	return true;
}
