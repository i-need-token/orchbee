import { z } from 'zod';

const protocolComponentSchema = z.number().int().min(0).max(65_535);
// Cross-language contract: search for `terminal-host-version-wire-limit`
// before changing this value, then update every occurrence together.
const MAX_HOST_VERSION_BYTES = 64;

export const SemanticVersionTextSchema = z
	.string()
	.min(5)
	.max(MAX_HOST_VERSION_BYTES)
	.regex(
		/^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/,
		'Expected a semantic version',
	);

export const ProtocolVersionSchema = z.strictObject({
	major: protocolComponentSchema,
	minor: protocolComponentSchema,
	patch: protocolComponentSchema,
});

export type ProtocolVersion = z.infer<typeof ProtocolVersionSchema>;

// Generation 1, encoded in the major component with minor and patch zero. Keep this synchronized
// with schemas/terminal/v1/constants.json, Rust and Zig. After the first public release, changes
// also update schemas/terminal/docs/version-history.md in the same change.
export const CURRENT_PROTOCOL_VERSION = {
	major: 1,
	minor: 0,
	patch: 0,
} as const satisfies ProtocolVersion;
