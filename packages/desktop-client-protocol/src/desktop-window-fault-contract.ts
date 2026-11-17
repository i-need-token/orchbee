import { z } from 'zod';

/**
 * The page posts each fault it contained, or that escaped it, to its own window, and the preload
 * forwards it to Main on this channel, so it reaches the daily diagnostic log. Only the site
 * travels: a closed token, never the error's message or stack.
 */
export const DESKTOP_WINDOW_FAULT_WINDOW_MESSAGE = 'orchbee.desktop.window-fault';
export const DESKTOP_WINDOW_FAULT_IPC_CHANNEL = 'orchbee.desktop.window-fault';

/**
 * Where a window fault happened: a site the page contained it to, or `uncaught-error` and
 * `unhandled-rejection` for one nothing contained. The page's own site list must stay within it.
 */
const DESKTOP_WINDOW_FAULT_SITES = [
	'command-decision',
	'connection-end-listener',
	'message-dispatch',
	'publication-apply',
	'session-end-listener',
	'snapshot-listener',
	'source-listener',
	'subscription-closed',
	'subscription-frame',
	'subscription-gap',
	'terminal-poll-record',
	'terminal-record-stop',
	'terminal-recovery-stop',
	'terminal-step',
	'terminal-surface-render',
	'uncaught-error',
	'unhandled-rejection',
	'viewer-ready-listener',
	'workbench-render',
] as const;

export const DesktopWindowFaultSiteSchema = z.enum(DESKTOP_WINDOW_FAULT_SITES);

export const DesktopWindowFaultWindowMessageSchema = z.strictObject({
	site: DesktopWindowFaultSiteSchema,
	type: z.literal(DESKTOP_WINDOW_FAULT_WINDOW_MESSAGE),
});

export type DesktopWindowFaultSite = z.infer<typeof DesktopWindowFaultSiteSchema>;
