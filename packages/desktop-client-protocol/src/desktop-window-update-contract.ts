import { z } from 'zod';

/** A product version as the window pointer names one: 1 to 64 printable ASCII characters. */
const DesktopWindowVersionSchema = z.string().regex(/^[!-~]{1,64}$/u);

/**
 * What Main sends the page over the window-update port: the newer window version the window offers
 * to switch to, or null while it offers none, whenever that changes and once when the port opens;
 * and, after a request it could not carry out, that the update waits until no terminal runs.
 */
export const DesktopWindowUpdateMainMessageSchema = z.discriminatedUnion('type', [
	z.strictObject({
		type: z.literal('desktop-window-update-offered'),
		version: DesktopWindowVersionSchema.nullable(),
	}),
	z.strictObject({ type: z.literal('desktop-window-update-waiting') }),
]);

/**
 * What the page sends Main over the same port: a request to switch to the version Main last
 * offered, named so a request made for an earlier offer switches to nothing else.
 */
export const DesktopWindowUpdateRendererMessageSchema = z.strictObject({
	type: z.literal('desktop-window-update-requested'),
	version: DesktopWindowVersionSchema,
});

export type DesktopWindowUpdateMainMessage = z.infer<typeof DesktopWindowUpdateMainMessageSchema>;
export type DesktopWindowUpdateRendererMessage = z.infer<
	typeof DesktopWindowUpdateRendererMessageSchema
>;
