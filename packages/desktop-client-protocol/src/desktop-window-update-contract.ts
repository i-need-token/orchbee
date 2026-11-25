import {
	TerminalPresentationIdSchema,
	WorkbenchWorkspaceIdSchema,
} from '@orchbee/workbench-client-protocol';
import { z } from 'zod';

/** A product version as the window pointer names one: 1 to 64 printable ASCII characters. */
const DesktopWindowVersionSchema = z.string().regex(/^[!-~]{1,64}$/u);

/** The identity of one place request, which its reply names: 1 to 64 printable ASCII characters. */
const DesktopWindowRequestIdSchema = z.string().regex(/^[!-~]{1,64}$/u);

/**
 * Where a relaunched window opens: the terminal the window displayed, the one renderer reload
 * recovery keeps, or without one the Workspace it had selected.
 */
export const DesktopRelaunchPlaceSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('terminal'), presentationId: TerminalPresentationIdSchema }),
	z.strictObject({ kind: z.literal('workspace'), workspaceId: WorkbenchWorkspaceIdSchema }),
]);

/**
 * Where the Session's move onto this window's version stands while its Server runs an older
 * version: `updating` while Main moves it, or why it waits: it runs a background program its Worker
 * would end, a connected Client cannot come back to a new Server by itself, a Terminal Host must be
 * replaced, which restarts every terminal and so waits for the person, or another reason, such as
 * the move failing.
 */
const DesktopSessionUpdateStateSchema = z.enum([
	'background-programs',
	'clients',
	'host-required',
	'other',
	'updating',
]);

/**
 * What Update offers: a newer window `version` to switch to, or, while the Session runs
 * `sessionVersion`, older than this window, a move of the Session to this window's own `version`,
 * with where that move stands.
 */
const DesktopWindowUpdateOfferSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('window'), version: DesktopWindowVersionSchema }),
	z.strictObject({
		kind: z.literal('session'),
		state: DesktopSessionUpdateStateSchema,
		sessionVersion: DesktopWindowVersionSchema,
		version: DesktopWindowVersionSchema,
	}),
]);

/**
 * What Main sends the page over the window-update port: what Update offers, or null while it offers
 * nothing, whenever that changes and once when the port opens; after a request it could not carry
 * out, either that the update waits, until no terminal runs for a newer window or for the reason a
 * Session offer names, or that launching the newer window failed; after a request whose Terminal
 * Host must be replaced, the question the page asks before every terminal restarts, with the
 * `terminals` that could not move off it once the Session runs the newer version, or null when it
 * asks before the Session restarts; once a confirmed replacement stopped the Session for this
 * window's own version, that the page loads again, which starts the Session; or a request for the
 * place the window shows, which the page answers with a reply naming the same `requestId`.
 */
export const DesktopWindowUpdateMainMessageSchema = z.discriminatedUnion('type', [
	z.strictObject({
		offer: DesktopWindowUpdateOfferSchema.nullable(),
		type: z.literal('desktop-window-update-offered'),
	}),
	z.strictObject({ type: z.literal('desktop-window-update-waiting') }),
	z.strictObject({ type: z.literal('desktop-window-update-reload') }),
	z.strictObject({ type: z.literal('desktop-window-update-failed') }),
	z.strictObject({
		terminals: z.number().int().nonnegative().nullable(),
		type: z.literal('desktop-window-update-host-replacement'),
	}),
	z.strictObject({
		requestId: DesktopWindowRequestIdSchema,
		type: z.literal('desktop-window-update-place-requested'),
	}),
]);

/**
 * What the page sends Main over the same port: a request to carry out what Main last offered,
 * naming its version so a request made for an earlier offer does nothing else, with the place the
 * window shows, which a relaunched window opens at; a request to refresh the offer from disk
 * once an update check settles; the person's answer to a Host replacement's question; or the reply
 * to a place request, with the place the window shows (null for none), naming that request by
 * `requestId`.
 */
export const DesktopWindowUpdateRendererMessageSchema = z.discriminatedUnion('type', [
	z.strictObject({
		place: DesktopRelaunchPlaceSchema.nullable(),
		type: z.literal('desktop-window-update-requested'),
		version: DesktopWindowVersionSchema,
	}),
	z.strictObject({
		type: z.literal('desktop-window-update-refresh'),
	}),
	z.strictObject({
		confirmed: z.boolean(),
		type: z.literal('desktop-window-update-host-replacement-answer'),
	}),
	z.strictObject({
		place: DesktopRelaunchPlaceSchema.nullable(),
		requestId: DesktopWindowRequestIdSchema,
		type: z.literal('desktop-window-update-place'),
	}),
]);

export type DesktopRelaunchPlace = z.infer<typeof DesktopRelaunchPlaceSchema>;
export type DesktopSessionUpdateState = z.infer<typeof DesktopSessionUpdateStateSchema>;
export type DesktopWindowUpdateOffer = z.infer<typeof DesktopWindowUpdateOfferSchema>;
export type DesktopWindowUpdateMainMessage = z.infer<typeof DesktopWindowUpdateMainMessageSchema>;
export type DesktopWindowUpdateRendererMessage = z.infer<
	typeof DesktopWindowUpdateRendererMessageSchema
>;
