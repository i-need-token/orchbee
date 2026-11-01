import { z } from 'zod';

export const DESKTOP_SERVER_CONNECTION_IPC_CHANNEL = 'orchbee.desktop.server-connection';
export const DESKTOP_SERVER_CONNECTION_WINDOW_MESSAGE = 'orchbee.desktop.server-connection-ready';

/** What only Main can know about the document it is connecting. */
const DesktopServerConnectionBootstrapSchema = z.strictObject({
	/** The first document of a window on a Session with no sign of damage may open a Workspace. */
	automaticWorkspaceAllowed: z.boolean(),
	defaultDirectory: z.string().min(1),
	terminalHostName: z.string().min(1),
});

/** Announces the one transferred port that carries this document's Client connection bytes. */
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
