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

/** What only Main can know about the document it is connecting. */
const DesktopServerConnectionBootstrapSchema = z.strictObject({
	/** The first document of a window on a Session with no sign of damage may open a Workspace. */
	automaticWorkspaceAllowed: z.boolean(),
	defaultDirectory: z.string().min(1),
	/** Whether this machine's Host can start the system shell as a login shell; Windows cannot. */
	loginShellAvailable: z.boolean(),
	/**
	 * Whether starting the Session was refused until it is explicitly recovered. Its connection then
	 * ends before anything is read, and only Main saw why.
	 */
	sessionRecoveryRequired: z.boolean(),
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
