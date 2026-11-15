/**
 * The rules that decide whether the window offers Update: where it runs from, what the window
 * pointer names, and which version a fallback start came back from. Each is a pure function of its
 * inputs, so the window's Ready state follows only from them.
 */
import path from 'node:path';

import { z } from 'zod';

import { isNewerProductVersion } from '../product-version';

/** The switch a window launcher adds when it starts the previous window version after a failure. */
// Cross-build contract: every window launcher from the window self-update release on passes it.
const FALLBACK_FROM_SWITCH = '--fallback-from-v1=';
const VERSIONS_DIRECTORY = 'versions';
const DESKTOP_DIRECTORY = 'desktop';

const NUMERIC_IDENTIFIER = '(?:0|[1-9]\\d*)';
const PRE_RELEASE_IDENTIFIER = `(?:${NUMERIC_IDENTIFIER}|\\d*[A-Za-z-][0-9A-Za-z-]*)`;
/**
 * A release version as the store writes one: `major.minor.patch`, an optional pre-release, no build
 * metadata, at most 64 characters (`crates/release-records/src/tokens.rs`).
 */
const RELEASE_VERSION = new RegExp(
	`^${NUMERIC_IDENTIFIER}\\.${NUMERIC_IDENTIFIER}\\.${NUMERIC_IDENTIFIER}` +
		`(?:-${PRE_RELEASE_IDENTIFIER}(?:\\.${PRE_RELEASE_IDENTIFIER})*)?$`,
	'u',
);

const ReleaseVersionSchema = z.string().max(64).regex(RELEASE_VERSION);

const PointerEntrySchema = z.strictObject({
	manifest: z.string().regex(/^[0-9a-f]{64}$/u),
	product: ReleaseVersionSchema,
});

/**
 * The window pointer `desktop\active.json`, in the store pointer's format 1
 * (`crates/release-records/src/pointer.rs`): `active` names the active window version.
 */
const WindowPointerSchema = z.strictObject({
	active: PointerEntrySchema,
	extensions: z.record(z.string(), z.unknown()).optional(),
	previous: PointerEntrySchema.optional(),
	version: z.literal(1),
});

/**
 * The window's `desktop\` folder when it runs from `desktop\versions\<its own version>\`, the only
 * place a window can switch versions from; null anywhere else, such as a flat layout or a build's
 * output folder, and always for a development build.
 */
export function locateWindowDesktopRoot(input: {
	readonly development: boolean;
	readonly executablePath: string;
	readonly ownVersion: string;
	readonly platform: NodeJS.Platform;
}): string | null {
	if (input.development) return null;
	const paths = input.platform === 'win32' ? path.win32 : path.posix;
	const sameName = (name: string, expected: string): boolean =>
		input.platform === 'win32' ? name.toLowerCase() === expected : name === expected;
	const versionDirectory = paths.dirname(input.executablePath);
	const versionsDirectory = paths.dirname(versionDirectory);
	const desktopRoot = paths.dirname(versionsDirectory);
	return paths.isAbsolute(input.executablePath) &&
		paths.basename(versionDirectory) === input.ownVersion &&
		sameName(paths.basename(versionsDirectory), VERSIONS_DIRECTORY) &&
		sameName(paths.basename(desktopRoot), DESKTOP_DIRECTORY)
		? desktopRoot
		: null;
}

/** The version a `--fallback-from-v1=<version>` argument names, or null without a valid one. */
export function readFallbackFromVersion(argumentsList: readonly string[]): string | null {
	const argument = argumentsList.find((candidate) => candidate.startsWith(FALLBACK_FROM_SWITCH));
	if (argument === undefined) return null;
	const version = ReleaseVersionSchema.safeParse(argument.slice(FALLBACK_FROM_SWITCH.length));
	return version.success ? version.data : null;
}

/** The active window version a window pointer names, or null when the text is no such pointer. */
export function parseWindowPointer(text: string): string | null {
	let document: unknown;
	try {
		document = JSON.parse(text);
	} catch {
		return null;
	}
	const pointer = WindowPointerSchema.safeParse(document);
	return pointer.success ? pointer.data.active.product : null;
}

/**
 * The version the window offers to switch to: the active window version when it is higher than the
 * window's own by Semantic Versioning precedence and is not the one a fallback start came back
 * from; otherwise null.
 */
export function resolveWindowUpdateOffer(input: {
	readonly activeVersion: string | null;
	readonly fallbackFrom: string | null;
	readonly ownVersion: string;
}): string | null {
	const { activeVersion, fallbackFrom, ownVersion } = input;
	if (activeVersion === null || activeVersion === fallbackFrom) return null;
	return isNewerProductVersion(activeVersion, ownVersion) ? activeVersion : null;
}
