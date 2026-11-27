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
const FALLBACK_FROM_SWITCH = '--fallback-from=';
const VERSIONS_DIRECTORY = 'versions';
const DESKTOP_DIRECTORY = 'desktop';
/** The bounds of the macOS and Linux store root rule (`crates/local-runtime/src/release_store.rs`). */
const STORE_ROOT_LIMIT = 1024;
const STORE_ROOT_REFUSED = /[:$`\\"'\p{Cc}]/u;

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
 * The release store root on macOS and Linux, by the rule the Client reads: `ORCHBEE_INSTALL_DIR`,
 * or `$HOME/.local/share/<stem>`, which is `orchbee` for release. Null without `HOME`, and for a
 * path the rule refuses: a relative one, one that spells a `.` or `..` component, or one longer
 * than 1024 bytes or holding `:`, `$`, a backquote, a backslash, a quote or a control character.
 */
export function resolvePosixReleaseStoreRoot(
	environment: Readonly<Record<string, string | undefined>>,
	artifactStem: string,
): string | null {
	const configured = environment['ORCHBEE_INSTALL_DIR'];
	const home = environment['HOME'];
	let root: string;
	if (configured !== undefined) root = configured;
	else if (home === undefined) return null;
	else root = path.posix.join(home, '.local', 'share', artifactStem);
	return path.posix.isAbsolute(root) &&
		new TextEncoder().encode(root).length <= STORE_ROOT_LIMIT &&
		!STORE_ROOT_REFUSED.test(root) &&
		!root.split('/').some((part) => part === '.' || part === '..')
		? root
		: null;
}

/**
 * The application bundle a macOS window runs from, `<name>.app` when its program is
 * `<name>.app/Contents/MacOS/<program>`; null for any other path.
 */
export function locateApplicationBundle(executablePath: string): string | null {
	const programs = path.posix.dirname(executablePath);
	const contents = path.posix.dirname(programs);
	const bundle = path.posix.dirname(contents);
	return path.posix.isAbsolute(executablePath) &&
		path.posix.basename(programs) === 'MacOS' &&
		path.posix.basename(contents) === 'Contents' &&
		path.posix.extname(bundle) === '.app'
		? bundle
		: null;
}

/**
 * The resources folder of a window version under `desktop`: `versions\<version>\resources` on
 * Windows, and on macOS the resources of the application bundle the version folder holds,
 * `versions/<version>/<application>.app/Contents/Resources`.
 */
export function windowVersionResources(input: {
	readonly applicationName: string;
	readonly desktopRoot: string;
	readonly platform: NodeJS.Platform;
	readonly version: string;
}): string {
	const paths = input.platform === 'win32' ? path.win32 : path.posix;
	const version = paths.join(input.desktopRoot, VERSIONS_DIRECTORY, input.version);
	return input.platform === 'darwin'
		? paths.join(version, `${input.applicationName}.app`, 'Contents', 'Resources')
		: paths.join(version, 'resources');
}

/**
 * The window's `desktop` folder, the only place a window can switch versions from, and null for a
 * development build. On Windows the window must run from `desktop\versions\<its own version>\`, so
 * a flat layout or a build's output folder offers nothing. On macOS the application bundle is the
 * entry: a window in a bundle reads `desktop` under the store root, `storeRoot`, unless the bundle
 * is one of the store's own copies there, which the launcher would otherwise replace. On Linux a
 * window in `desktop/versions/<its own version>/` reads that `desktop`, as on Windows, and a window
 * outside the store's `desktop`, such as the AppImage's or the Debian package's, reads the store's,
 * whose entry it relaunches through.
 */
export function locateWindowDesktopRoot(input: {
	readonly development: boolean;
	readonly executablePath: string;
	readonly ownVersion: string;
	readonly platform: NodeJS.Platform;
	/** On macOS and Linux, the store root (`resolvePosixReleaseStoreRoot`); unused on Windows. */
	readonly storeRoot: string | null;
}): string | null {
	if (input.development) return null;
	if (input.platform === 'darwin') {
		const application = locateApplicationBundle(input.executablePath);
		if (input.storeRoot === null || application === null) return null;
		const desktopRoot = path.posix.join(input.storeRoot, DESKTOP_DIRECTORY);
		// APFS compares names without letter case by default.
		const relative = path.posix.relative(desktopRoot.toLowerCase(), application.toLowerCase());
		const outside = relative === '..' || relative.startsWith('../');
		return outside ? desktopRoot : null;
	}
	const paths = input.platform === 'win32' ? path.win32 : path.posix;
	const sameName = (name: string, expected: string): boolean =>
		input.platform === 'win32' ? name.toLowerCase() === expected : name === expected;
	if (!paths.isAbsolute(input.executablePath)) return null;
	const versionDirectory = paths.dirname(input.executablePath);
	const versionsDirectory = paths.dirname(versionDirectory);
	const desktopRoot = paths.dirname(versionsDirectory);
	if (
		paths.basename(versionDirectory) === input.ownVersion &&
		sameName(paths.basename(versionsDirectory), VERSIONS_DIRECTORY) &&
		sameName(paths.basename(desktopRoot), DESKTOP_DIRECTORY)
	) {
		return desktopRoot;
	}
	if (input.platform !== 'linux' || input.storeRoot === null) return null;
	const storeDesktopRoot = path.posix.join(input.storeRoot, DESKTOP_DIRECTORY);
	const relative = path.posix.relative(storeDesktopRoot, input.executablePath);
	return relative === '..' || relative.startsWith('../') ? storeDesktopRoot : null;
}

/** The version a `--fallback-from=<version>` argument names, or null without a valid one. */
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
