import { z } from 'zod';

import { DesktopServerConnectionDocumentSchema } from './desktop-server-connection-contract';

/**
 * A remote Session connection for one saved machine whose Workspaces the page shows: the page asks
 * its preload, the preload asks Main naming its document, and Main starts that machine's session
 * relay and answers with the relay's port or why there is none.
 */
export const DESKTOP_MACHINE_SESSION_WINDOW_REQUEST = 'orchbee.desktop.machine-session-request';
export const DESKTOP_MACHINE_SESSION_REQUEST_IPC_CHANNEL =
	'orchbee.desktop.machine-session-request';
export const DESKTOP_MACHINE_SESSION_IPC_CHANNEL = 'orchbee.desktop.machine-session';
export const DESKTOP_MACHINE_SESSION_WINDOW_MESSAGE = 'orchbee.desktop.machine-session-ready';

/** A saved machine's catalog id. */
const MachineIdSchema = z.string().regex(/^[0-9a-f]{32}$/u);

/** The page's number for one request, which its answer repeats. */
const RequestSchema = z.number().int().positive().max(Number.MAX_SAFE_INTEGER);

const CapabilityNameSchema = z.string().regex(/^[a-z0-9-]{1,64}$/u);

/**
 * Why no connection was made: the relay's own reason, or Main's: `timeout` for a relay that said
 * nothing within its bound, `protocol` for one that said something else, and `unavailable` while
 * Desktop offers no remote machines or the document already holds that machine's connection.
 */
const DesktopMachineSessionRefusalSchema = z.enum([
	'client-too-old',
	'development-build-mismatch',
	'hub-unavailable',
	'link-lost',
	'not-found',
	'other-environment',
	'protocol',
	'session-too-old',
	'timeout',
	'transient',
	'unavailable',
]);

export const DesktopMachineSessionWindowRequestSchema = z.strictObject({
	machine: MachineIdSchema,
	request: RequestSchema,
	type: z.literal(DESKTOP_MACHINE_SESSION_WINDOW_REQUEST),
});

/** The preload's request to Main, naming the document the page never sees. */
export const DesktopMachineSessionRequestSchema = z.strictObject({
	document: DesktopServerConnectionDocumentSchema,
	machine: MachineIdSchema,
	request: RequestSchema,
});

/** What the page derives a remote Session's bootstrap from; the rest is the page's own. */
const DesktopMachineSessionBootstrapSchema = z.strictObject({
	machine: MachineIdSchema,
	/** Every capability the remote Session's handshake answer listed. */
	negotiatedCapabilities: z.array(CapabilityNameSchema).max(256),
});

const ADMITTED = {
	bootstrap: DesktopMachineSessionBootstrapSchema,
	outcome: z.literal('admitted'),
	request: RequestSchema,
};

const REFUSED = {
	outcome: z.literal('refused'),
	reason: DesktopMachineSessionRefusalSchema,
	request: RequestSchema,
};

/** Main's answer; an admitted one transfers exactly one port, the relay's bytes. */
export const DesktopMachineSessionReplySchema = z.discriminatedUnion('outcome', [
	z.strictObject({ ...ADMITTED, document: DesktopServerConnectionDocumentSchema }),
	z.strictObject({ ...REFUSED, document: DesktopServerConnectionDocumentSchema }),
]);

const WINDOW_MESSAGE_TYPE = z.literal(DESKTOP_MACHINE_SESSION_WINDOW_MESSAGE);

/** The answer as the page receives it, with an admitted connection's port transferred. */
export const DesktopMachineSessionWindowMessageSchema = z.discriminatedUnion('outcome', [
	z.strictObject({ ...ADMITTED, type: WINDOW_MESSAGE_TYPE }),
	z.strictObject({ ...REFUSED, type: WINDOW_MESSAGE_TYPE }),
]);

export type DesktopMachineSessionReply = z.infer<typeof DesktopMachineSessionReplySchema>;
export type DesktopMachineSessionWindowMessage = z.infer<
	typeof DesktopMachineSessionWindowMessageSchema
>;
export type DesktopMachineSessionRefusal = z.infer<typeof DesktopMachineSessionRefusalSchema>;
