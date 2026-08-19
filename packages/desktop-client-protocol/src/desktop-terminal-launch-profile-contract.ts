import { z } from 'zod';

const MAXIMUM_ARGUMENT_BYTES = 32 * 1024;
const MAXIMUM_ARGUMENT_COUNT = 128;
const MAXIMUM_CATALOG_BYTES = 1024 * 1024;
const MAXIMUM_EXECUTABLE_BYTES = 32 * 1024;
const MAXIMUM_PROFILE_COUNT = 64;
const MAXIMUM_PROFILE_NAME_BYTES = 512;
const utf8Encoder = new TextEncoder();

function containsUnpairedSurrogate(value: string): boolean {
	for (let index = 0; index < value.length; index += 1) {
		const code = value.charCodeAt(index);
		if (code >= 0xd800 && code <= 0xdbff) {
			const next = value.charCodeAt(index + 1);
			if (next >= 0xdc00 && next <= 0xdfff) {
				index += 1;
				continue;
			}
			return true;
		}
		if (code >= 0xdc00 && code <= 0xdfff) return true;
	}
	return false;
}

export const DESKTOP_TERMINAL_LAUNCH_PROFILE_LIMITS = Object.freeze({
	maximumArgumentBytes: MAXIMUM_ARGUMENT_BYTES,
	maximumArgumentCount: MAXIMUM_ARGUMENT_COUNT,
	maximumCatalogBytes: MAXIMUM_CATALOG_BYTES,
	maximumExecutableBytes: MAXIMUM_EXECUTABLE_BYTES,
	maximumProfileCount: MAXIMUM_PROFILE_COUNT,
	maximumProfileNameBytes: MAXIMUM_PROFILE_NAME_BYTES,
});

function boundedText(maximumBytes: number, label: string): z.ZodString {
	return z
		.string()
		.max(maximumBytes)
		.refine(
			(value) => utf8Encoder.encode(value).byteLength <= maximumBytes,
			`${label} exceeds ${maximumBytes} UTF-8 bytes`,
		)
		.refine((value) => !containsUnpairedSurrogate(value), `${label} contains invalid Unicode`)
		.refine((value) => !value.includes('\0'), `${label} cannot contain NUL bytes`);
}

export const DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID = 'system-default';

export const DesktopTerminalLaunchProfileIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u)
	.brand<'DesktopTerminalLaunchProfileId'>();

export const DesktopTerminalLaunchProfileNameSchema = boundedText(
	MAXIMUM_PROFILE_NAME_BYTES,
	'Profile name',
)
	.trim()
	.min(1);

export const DesktopTerminalLaunchProfileCommandSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('system-shell'),
		login: z.boolean(),
	}),
	z.strictObject({
		arguments: z
			.array(boundedText(MAXIMUM_ARGUMENT_BYTES, 'Launch argument'))
			.max(MAXIMUM_ARGUMENT_COUNT),
		executable: boundedText(MAXIMUM_EXECUTABLE_BYTES, 'Executable path').min(1),
		kind: z.literal('custom-executable'),
	}),
]);

export const DesktopTerminalLaunchProfileSchema = z
	.strictObject({
		command: DesktopTerminalLaunchProfileCommandSchema,
		id: DesktopTerminalLaunchProfileIdSchema,
		name: DesktopTerminalLaunchProfileNameSchema.nullable(),
	})
	.superRefine((profile, context) => {
		if (profile.name === null && profile.id !== DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID) {
			context.addIssue({
				code: 'custom',
				message: 'Only the built-in profile can use the localized label marker',
				path: ['name'],
			});
		}
	});

export const DesktopTerminalLaunchProfileCatalogSchema = z
	.strictObject({
		defaultProfileId: DesktopTerminalLaunchProfileIdSchema,
		profiles: z.array(DesktopTerminalLaunchProfileSchema).min(1).max(MAXIMUM_PROFILE_COUNT),
		version: z.literal(1),
	})
	.superRefine((catalog, context) => {
		const identifiers = new Set<string>();
		for (const [index, profile] of catalog.profiles.entries()) {
			if (identifiers.has(profile.id)) {
				context.addIssue({
					code: 'custom',
					message: 'Profile IDs must be unique',
					path: ['profiles', index, 'id'],
				});
			}
			identifiers.add(profile.id);
		}
		if (!identifiers.has(catalog.defaultProfileId)) {
			context.addIssue({
				code: 'custom',
				message: 'The default profile ID must identify a current profile',
				path: ['defaultProfileId'],
			});
		}
		if (utf8Encoder.encode(JSON.stringify(catalog)).byteLength > MAXIMUM_CATALOG_BYTES) {
			context.addIssue({
				code: 'custom',
				message: `Profile catalog exceeds ${MAXIMUM_CATALOG_BYTES} UTF-8 bytes`,
				path: [],
			});
		}
	});

export const DesktopTerminalLaunchProfilePersistenceIssueSchema = z.enum([
	'invalid-record',
	'read-failed',
	'write-failed',
]);

export const DesktopTerminalLaunchProfileSnapshotSchema = z.strictObject({
	capabilities: z.strictObject({
		loginShell: z.boolean(),
	}),
	catalog: DesktopTerminalLaunchProfileCatalogSchema,
	persistenceIssue: DesktopTerminalLaunchProfilePersistenceIssueSchema.nullable(),
	source: z.enum(['built-in', 'stored']),
});

export const DesktopTerminalLaunchProfileRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopTerminalLaunchProfileRequestId'>();

const requestIdentityShape = {
	requestId: DesktopTerminalLaunchProfileRequestIdSchema,
} as const;

export const DesktopTerminalLaunchProfileRendererCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		...requestIdentityShape,
		type: z.literal('desktop-terminal-launch-profile-snapshot-requested'),
	}),
	z.strictObject({
		...requestIdentityShape,
		name: DesktopTerminalLaunchProfileNameSchema,
		sourceProfileId: DesktopTerminalLaunchProfileIdSchema,
		type: z.literal('desktop-terminal-launch-profile-create-requested'),
	}),
	z.strictObject({
		...requestIdentityShape,
		profile: DesktopTerminalLaunchProfileSchema,
		type: z.literal('desktop-terminal-launch-profile-update-requested'),
	}),
	z.strictObject({
		...requestIdentityShape,
		profileId: DesktopTerminalLaunchProfileIdSchema,
		type: z.literal('desktop-terminal-launch-profile-default-requested'),
	}),
	z.strictObject({
		...requestIdentityShape,
		profileId: DesktopTerminalLaunchProfileIdSchema,
		type: z.literal('desktop-terminal-launch-profile-delete-requested'),
	}),
	z.strictObject({
		...requestIdentityShape,
		type: z.literal('desktop-terminal-launch-profile-reset-requested'),
	}),
]);

export const DesktopTerminalLaunchProfileFailureCodeSchema = z.enum([
	'busy',
	'catalog-full',
	'control-unavailable',
	'default-profile-delete',
	'identity-allocation-failed',
	'invalid-profile',
	'last-profile-delete',
	'profile-not-found',
	'unsupported-platform',
	'write-failed',
]);

const desktopTerminalLaunchProfileResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('completed'),
		snapshot: DesktopTerminalLaunchProfileSnapshotSchema,
	}),
	z.strictObject({
		code: DesktopTerminalLaunchProfileFailureCodeSchema,
		kind: z.literal('failed'),
		snapshot: DesktopTerminalLaunchProfileSnapshotSchema,
	}),
]);

export const DesktopTerminalLaunchProfileMainEventSchema = z.strictObject({
	requestId: DesktopTerminalLaunchProfileRequestIdSchema,
	result: desktopTerminalLaunchProfileResultSchema,
	type: z.literal('desktop-terminal-launch-profile-responded'),
});

export type DesktopTerminalLaunchProfile = z.infer<typeof DesktopTerminalLaunchProfileSchema>;
export type DesktopTerminalLaunchProfileCatalog = z.infer<
	typeof DesktopTerminalLaunchProfileCatalogSchema
>;
export type DesktopTerminalLaunchProfileCommand = z.infer<
	typeof DesktopTerminalLaunchProfileCommandSchema
>;
export type DesktopTerminalLaunchProfileFailureCode = z.infer<
	typeof DesktopTerminalLaunchProfileFailureCodeSchema
>;
export type DesktopTerminalLaunchProfileId = z.infer<typeof DesktopTerminalLaunchProfileIdSchema>;
export type DesktopTerminalLaunchProfileMainEvent = z.infer<
	typeof DesktopTerminalLaunchProfileMainEventSchema
>;
export type DesktopTerminalLaunchProfilePersistenceIssue = z.infer<
	typeof DesktopTerminalLaunchProfilePersistenceIssueSchema
>;
export type DesktopTerminalLaunchProfileRendererCommand = z.infer<
	typeof DesktopTerminalLaunchProfileRendererCommandSchema
>;
export type DesktopTerminalLaunchProfileRequestId = z.infer<
	typeof DesktopTerminalLaunchProfileRequestIdSchema
>;
export type DesktopTerminalLaunchProfileSnapshot = z.infer<
	typeof DesktopTerminalLaunchProfileSnapshotSchema
>;
