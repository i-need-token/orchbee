import { z } from 'zod';

import { DesktopRelaunchPlaceSchema } from './desktop-window-update-contract';

/** The page asks once it is listening, so a connection never reaches a document not yet ready for it. */
export const DESKTOP_SERVER_CONNECTION_WINDOW_REQUEST = 'orchbee.desktop.server-connection-request';
export const DESKTOP_SERVER_CONNECTION_REQUEST_IPC_CHANNEL =
	'orchbee.desktop.server-connection-request';
export const DESKTOP_SERVER_CONNECTION_IPC_CHANNEL = 'orchbee.desktop.server-connection';
export const DESKTOP_SERVER_CONNECTION_WINDOW_MESSAGE = 'orchbee.desktop.server-connection-ready';

export const DesktopServerConnectionWindowRequestSchema = z.strictObject({
	type: z.literal(DESKTOP_SERVER_CONNECTION_WINDOW_REQUEST),
});

/**
 * Names the asking document. A window's next document can receive what Main meant for the last one,
 * so an answer has to say whose it is.
 */
export const DesktopServerConnectionDocumentSchema = z.uuid();

/** A product version a Session or Client states: 1 to 64 printable ASCII characters, no space. */
const ProductVersionSchema = z.string().regex(/^[!-~]{1,64}$/u);

/**
 * A process an earlier start registered that is still running, which keeps the Session from
 * recovering until it ends, as `server start` names it: its role, the file name of its program,
 * never a path, and its process ID.
 */
export const DesktopBlockingProcessSchema = z.strictObject({
	processId: z.number().int().min(1).max(4_294_967_295),
	program: z
		.string()
		.regex(/^(?:orchbee|ob-(?:dev|test)-[0-9a-f]{16})-(?:server|worker|terminal-host)(?:\.exe)?$/u),
	role: z.enum(['server', 'worker', 'terminal-host']),
});

/** The `ioKind` vocabulary `server start` names the kind of a system error with. */
export const DesktopSystemErrorKindSchema = z.enum([
	'not-found',
	'permission-denied',
	'invalid-input',
	'invalid-data',
	'timed-out',
	'unsupported',
	'interrupted',
	'unexpected-eof',
	'would-block',
	'out-of-memory',
	'other',
]);

/**
 * The rule a refused data or socket folder, or a file in one, breaks, as the `refusal` member of a
 * `server start` failure names it.
 */
export const DesktopPathRefusalCauseSchema = z.enum([
	'access-control-list',
	'locks-unsupported',
	'other-owner',
	'others-can-change',
	'others-can-read',
	'permissions-not-kept',
	'symbolic-link',
	'unsafe-socket-folder',
]);

/**
 * Why starting the Session was refused. Each reason has its own message; a refused version names
 * this Desktop's version and the Session's, which a Session older than the handshake never states.
 * A recovery refusal names the process that blocks it when the start found one, a Server the
 * system would not create keeps what the system answered, and a refused folder names the item at
 * fault and the rule it breaks.
 */
const DesktopSessionRefusalSchema = z.discriminatedUnion('reason', [
	z.strictObject({
		blockingProcess: DesktopBlockingProcessSchema.optional().catch(undefined),
		reason: z.literal('recovery-required'),
	}),
	z.strictObject({
		ioKind: DesktopSystemErrorKindSchema,
		/** The system's own error number, or null when it stated none. */
		osError: z.number().int().nullable(),
		reason: z.literal('server-not-started'),
	}),
	z.strictObject({
		desktopVersion: ProductVersionSchema,
		reason: z.literal('desktop-too-old'),
		sessionVersion: ProductVersionSchema.nullable(),
	}),
	z.strictObject({
		desktopVersion: ProductVersionSchema,
		reason: z.literal('session-too-old'),
		/** `orchbee server restart` can replace the Session's Server here, keeping its terminals. */
		restartable: z.boolean(),
		sessionVersion: ProductVersionSchema.nullable(),
	}),
	z.strictObject({ reason: z.literal('other-environment') }),
	z.strictObject({ reason: z.literal('development-build-mismatch') }),
	z.strictObject({
		cause: DesktopPathRefusalCauseSchema,
		/** The refused folder or file, spelled as the TUI and CLI name it. */
		path: z.string().min(1).max(32_767),
		reason: z.literal('path-refused'),
		/** Whose rules refused it, which decides the remedy the words give. */
		system: z.enum(['posix', 'windows']),
	}),
]);

/** A capability name of the `client-server` handshake: 1 to 64 lowercase letters, digits, and hyphens. */
const CapabilityNameSchema = z.string().regex(/^[a-z0-9-]{1,64}$/u);

/**
 * A shell installed on this machine that Add profile lists; Windows shells are named by `kind`, and a
 * `named` one by `name`, its file name with its folder when two share it.
 */
const DesktopInstalledShellSchema = z.strictObject({
	arguments: z.array(z.string().max(32 * 1024)).max(128),
	executable: z.string().min(1).max(4096),
	kind: z.enum(['powershell', 'windows-powershell', 'command-prompt', 'git-bash', 'named']),
	name: z.string().max(512),
});

export type DesktopInstalledShell = z.infer<typeof DesktopInstalledShellSchema>;

/** What only Main can know about the document it is connecting. */
const DesktopServerConnectionBootstrapSchema = z.strictObject({
	/**
	 * The version this window runs, as About shows it; null when it is not a product version. A
	 * malformed value reads as unknown rather than refusing the document.
	 */
	applicationVersion: ProductVersionSchema.nullable().catch(null),
	/** The first document of a window on a Session with no sign of damage may open a Workspace. */
	automaticWorkspaceAllowed: z.boolean(),
	/**
	 * The final directory name of the checkout a development build came from, which the window marks
	 * beside its environment; null for a release and for a test build packaged from a home.
	 */
	checkoutName: z.string().min(1).nullable(),
	/**
	 * The person's home folder on this machine, where the Session's Server also runs: where a terminal
	 * without a directory starts, and what a Workspace folder under it shows as `~`.
	 */
	defaultDirectory: z.string().min(1),
	/**
	 * Which build this Desktop is. A release withholds the entry points of features not yet polished;
	 * development and test builds offer every one.
	 */
	distribution: z.enum(['development', 'release', 'test']),
	/**
	 * The shells installed on this machine, where the Session's Server also runs, that Add profile
	 * lists. A malformed list reads as empty, so Add profile asks for a name as before.
	 */
	installedShells: z.array(DesktopInstalledShellSchema).max(64).catch([]),
	/** Whether this machine's Host can start the system shell as a login shell; Windows cannot. */
	loginShellAvailable: z.boolean(),
	/**
	 * The capability names this document's `server start` reported its `client-server` handshake
	 * negotiated, which the window's actions that need one read; null when the answer named none or
	 * could not be read. A malformed list reads as null, so the actions it gates stay unavailable
	 * rather than the document being refused.
	 */
	negotiatedCapabilities: z.array(CapabilityNameSchema).max(256).nullable().catch(null),
	/**
	 * Whether Main serves Quit All, which a terminal offers once its Server gave up on it; false where
	 * Desktop keeps no notification-area or menu-bar icon.
	 */
	quitAllAvailable: z.boolean(),
	/**
	 * Where the first document of a window relaunched by Update opens, from the window's command
	 * line; null for every other document.
	 */
	relaunchPlace: DesktopRelaunchPlaceSchema.nullable(),
	/**
	 * Why starting the Session was refused, or null when it was not or the reason is unknown. Its
	 * connection then ends before anything is read, and only Main saw why. A refusal this window
	 * cannot read, such as a reason a newer Main names, reads as unknown, so the window gives its
	 * general sentence instead of refusing the document.
	 */
	sessionRefusal: DesktopSessionRefusalSchema.nullable().catch(null),
	terminalHostName: z.string().min(1),
	/**
	 * Whether Main follows a release store's window pointer, so it can ever offer this window a newer
	 * version: false for a window outside a store, such as a development build or a Linux window. A
	 * malformed value reads as false.
	 */
	windowUpdatesOffered: z.boolean().catch(false),
});

export const DesktopServerConnectionReplySchema = z.strictObject({
	bootstrap: DesktopServerConnectionBootstrapSchema,
	document: DesktopServerConnectionDocumentSchema,
});

/**
 * What the ports transferred with a reply carry, in this order: the document's Client connection
 * bytes, the native folder picker a new Project starts from, which only Main can open, the
 * summary of terminals and Agents the page reports for the notification-area or menu-bar icon, and
 * the newer window version Main offers to switch to.
 */
export const DESKTOP_SERVER_CONNECTION_PORT_ROLES = [
	'server-connection',
	'project-folder-acquisition',
	'background-status',
	'window-update',
] as const;

export type DesktopServerConnectionPortRole = (typeof DESKTOP_SERVER_CONNECTION_PORT_ROLES)[number];

/**
 * How long a window waits for a Server to serve its Session again once its connection ended without
 * Main or the page closing it, as the TUI does. Main reads the Session, starting nothing, and
 * reloads the window once a Server answers; the page says Reconnecting meanwhile, then Disconnected.
 */
export const DESKTOP_SESSION_REJOIN_MILLISECONDS = 150_000;

/**
 * The page's one message on the `server-connection` port that is not connection bytes: it has read
 * `byteLength` more bytes of what Main relayed, once each chunk's handling returns. Main relays
 * output only while what the page has not yet read stays under its bound, so a page that reads
 * slowly slows its own connection instead of growing Main.
 */
export const DesktopServerConnectionReadSchema = z.strictObject({
	byteLength: z.number().int().positive().max(Number.MAX_SAFE_INTEGER),
	type: z.literal('desktop-server-connection-read'),
});

export type DesktopServerConnectionRead = z.infer<typeof DesktopServerConnectionReadSchema>;

/** Announces this document's ports, transferred in `DESKTOP_SERVER_CONNECTION_PORT_ROLES` order. */
export const DesktopServerConnectionWindowMessageSchema = z.strictObject({
	bootstrap: DesktopServerConnectionBootstrapSchema,
	security: z.strictObject({
		contextIsolated: z.literal(true),
		sandboxed: z.literal(true),
	}),
	type: z.literal(DESKTOP_SERVER_CONNECTION_WINDOW_MESSAGE),
});

export type DesktopServerConnectionBootstrap = z.infer<
	typeof DesktopServerConnectionBootstrapSchema
>;
export type DesktopServerConnectionReply = z.infer<typeof DesktopServerConnectionReplySchema>;
export type DesktopSessionRefusal = z.infer<typeof DesktopSessionRefusalSchema>;
