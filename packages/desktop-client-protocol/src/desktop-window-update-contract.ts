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
 * What Main sends the page over the window-update port: the newer window version the window offers
 * to switch to, or null while it offers none, whenever that changes and once when the port opens;
 * after a request it could not carry out, either that the update waits until no terminal runs or
 * that launching the newer window failed; after a request whose Terminal Host must be replaced, the
 * question the page asks before every terminal restarts; or a request for the place the window
 * shows, which the page answers with a reply naming the same `requestId`.
 */
export const DesktopWindowUpdateMainMessageSchema = z.discriminatedUnion('type', [
	z.strictObject({
		type: z.literal('desktop-window-update-offered'),
		version: DesktopWindowVersionSchema.nullable(),
	}),
	z.strictObject({ type: z.literal('desktop-window-update-waiting') }),
	z.strictObject({ type: z.literal('desktop-window-update-failed') }),
	z.strictObject({ type: z.literal('desktop-window-update-host-replacement') }),
	z.strictObject({
		requestId: DesktopWindowRequestIdSchema,
		type: z.literal('desktop-window-update-place-requested'),
	}),
]);

/**
 * What the page sends Main over the same port: a request to switch to the version Main last
 * offered, named so a request made for an earlier offer switches to nothing else, with the place
 * the window shows, which the relaunched window opens at; a request to refresh the offer from disk
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
export type DesktopWindowUpdateMainMessage = z.infer<typeof DesktopWindowUpdateMainMessageSchema>;
export type DesktopWindowUpdateRendererMessage = z.infer<
	typeof DesktopWindowUpdateRendererMessageSchema
>;
