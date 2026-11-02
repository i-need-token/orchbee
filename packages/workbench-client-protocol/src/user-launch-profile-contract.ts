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

export const USER_LAUNCH_PROFILE_LIMITS = Object.freeze({
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

export const BUILT_IN_USER_LAUNCH_PROFILE_ID = 'system-default';

/** @internal Fixture parser for Web tests and Stories; production parses it only inside this module's catalog schemas. */
export const UserLaunchProfileIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u)
	.brand<'UserLaunchProfileId'>();

export const UserLaunchProfileNameSchema = boundedText(MAXIMUM_PROFILE_NAME_BYTES, 'Profile name')
	.refine((value) => value.trim() === value, 'Profile name must be trimmed')
	.min(1);

const UserLaunchProfileCommandSchema = z.discriminatedUnion('kind', [
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

export const UserLaunchProfileSchema = z
	.strictObject({
		command: UserLaunchProfileCommandSchema,
		id: UserLaunchProfileIdSchema,
		name: UserLaunchProfileNameSchema.nullable(),
	})
	.superRefine((profile, context) => {
		if (profile.name === null && profile.id !== BUILT_IN_USER_LAUNCH_PROFILE_ID) {
			context.addIssue({
				code: 'custom',
				message: 'Only the built-in profile can use the localized label marker',
				path: ['name'],
			});
		}
	});

export const UserLaunchProfileCatalogSchema = z
	.strictObject({
		defaultProfileId: UserLaunchProfileIdSchema,
		profiles: z.array(UserLaunchProfileSchema).min(1).max(MAXIMUM_PROFILE_COUNT),
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

export type UserLaunchProfile = z.infer<typeof UserLaunchProfileSchema>;
export type UserLaunchProfileCatalog = z.infer<typeof UserLaunchProfileCatalogSchema>;
export type UserLaunchProfileId = z.infer<typeof UserLaunchProfileIdSchema>;
