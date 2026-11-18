import {
	TerminalPresentationIdSchema,
	WorkbenchWorkspaceIdSchema,
} from '@orchbee/workbench-client-protocol';
import { z } from 'zod';

/** A product version as the window pointer names one: 1 to 64 printable ASCII characters. */
const DesktopWindowVersionSchema = z.string().regex(/^[!-~]{1,64}$/u);

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
 * and, after a request it could not carry out, either that the update waits until no terminal runs
 * or that launching the newer window failed.
 */
export const DesktopWindowUpdateMainMessageSchema = z.discriminatedUnion('type', [
	z.strictObject({
		type: z.literal('desktop-window-update-offered'),
		version: DesktopWindowVersionSchema.nullable(),
	}),
	z.strictObject({ type: z.literal('desktop-window-update-waiting') }),
	z.strictObject({ type: z.literal('desktop-window-update-failed') }),
]);

/**
 * What the page sends Main over the same port: a request to switch to the version Main last
 * offered, named so a request made for an earlier offer switches to nothing else, with the place
 * the window shows, which the relaunched window opens at.
 */
export const DesktopWindowUpdateRendererMessageSchema = z.strictObject({
	place: DesktopRelaunchPlaceSchema.nullable(),
	type: z.literal('desktop-window-update-requested'),
	version: DesktopWindowVersionSchema,
});

export type DesktopRelaunchPlace = z.infer<typeof DesktopRelaunchPlaceSchema>;
export type DesktopWindowUpdateMainMessage = z.infer<typeof DesktopWindowUpdateMainMessageSchema>;
export type DesktopWindowUpdateRendererMessage = z.infer<
	typeof DesktopWindowUpdateRendererMessageSchema
>;
