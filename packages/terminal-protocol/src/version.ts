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

// Keep this synchronized with Zig. Released changes (>= 1.0.0) also update
// docs/version-history.md in the same change.
export const CURRENT_PROTOCOL_VERSION = {
	major: 0,
	minor: 0,
	patch: 7,
} as const satisfies ProtocolVersion;
