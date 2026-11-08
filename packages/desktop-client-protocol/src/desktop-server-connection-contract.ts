import { z } from 'zod';

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
 * Why starting the Session was refused. Each reason has its own message; a refused version names
 * this Desktop's version and the Session's, which a Session older than the handshake never states.
 */
const DesktopSessionRefusalSchema = z.discriminatedUnion('reason', [
	z.strictObject({ reason: z.literal('recovery-required') }),
	z.strictObject({
		desktopVersion: ProductVersionSchema,
		reason: z.literal('desktop-too-old'),
		sessionVersion: ProductVersionSchema.nullable(),
	}),
	z.strictObject({
		desktopVersion: ProductVersionSchema,
		reason: z.literal('session-too-old'),
		sessionVersion: ProductVersionSchema.nullable(),
	}),
	z.strictObject({ reason: z.literal('other-environment') }),
	z.strictObject({ reason: z.literal('development-build-mismatch') }),
]);

/** What only Main can know about the document it is connecting. */
const DesktopServerConnectionBootstrapSchema = z.strictObject({
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
	/** Whether this machine's Host can start the system shell as a login shell; Windows cannot. */
	loginShellAvailable: z.boolean(),
	/**
	 * Why starting the Session was refused, or null when it was not or the reason is unknown. Its
	 * connection then ends before anything is read, and only Main saw why.
	 */
	sessionRefusal: DesktopSessionRefusalSchema.nullable(),
	terminalHostName: z.string().min(1),
});

export const DesktopServerConnectionReplySchema = z.strictObject({
	bootstrap: DesktopServerConnectionBootstrapSchema,
	document: DesktopServerConnectionDocumentSchema,
});

/**
 * What the ports transferred with a reply carry, in this order: the document's Client connection
 * bytes, and the native folder picker a new Project starts from, which only Main can open.
 */
export const DESKTOP_SERVER_CONNECTION_PORT_ROLES = [
	'server-connection',
	'project-folder-acquisition',
] as const;

export type DesktopServerConnectionPortRole = (typeof DESKTOP_SERVER_CONNECTION_PORT_ROLES)[number];

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
