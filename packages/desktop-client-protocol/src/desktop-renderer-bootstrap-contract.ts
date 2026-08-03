import { z } from 'zod';

import { DesktopClientSessionBootstrapSchema } from './desktop-client-session-contract';

export const DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL = 'orchbee.desktop.renderer-bootstrap';
export const DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE = 'orchbee.desktop.renderer-bootstrap-ready';

export const DESKTOP_RENDERER_PORT_ROLES = [
	'client-session',
	'terminal-projection',
	'application-lifetime',
] as const;

export const DesktopRendererBootstrapSchema = z.strictObject({
	clientSession: DesktopClientSessionBootstrapSchema,
	type: z.literal(DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL),
});

export const DesktopRendererBootstrapWindowMessageSchema = z.strictObject({
	bootstrap: DesktopRendererBootstrapSchema,
	security: z.strictObject({ contextIsolated: z.boolean(), sandboxed: z.boolean() }),
	type: z.literal(DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE),
});

export type DesktopRendererBootstrap = z.infer<typeof DesktopRendererBootstrapSchema>;
