import { z } from 'zod';

/** A count the page reports, bounded so a malformed message never reaches the tray. */
const DesktopBackgroundCountSchema = z.number().int().min(0).max(1_000_000);

/**
 * What the window's terminals and Agents amount to, as the page sees them. Ended and failed
 * terminals are not running; each Agent counts once, under the state it shows.
 */
const DesktopBackgroundStatusSummarySchema = z.strictObject({
	/** Agents waiting for the person. */
	attention: DesktopBackgroundCountSchema,
	/** Agents whose finished turn the person has not yet seen. */
	doneUnseen: DesktopBackgroundCountSchema,
	failed: DesktopBackgroundCountSchema,
	runningTerminals: DesktopBackgroundCountSchema,
	workingAgents: DesktopBackgroundCountSchema,
});

/**
 * What the page sends Main over the background-status port: the summary whenever it changes, and a
 * request to show the window, which a page cannot do for a window Main hid.
 */
export const DesktopBackgroundStatusRendererMessageSchema = z.discriminatedUnion('type', [
	z.strictObject({
		summary: DesktopBackgroundStatusSummarySchema,
		type: z.literal('desktop-background-status-changed'),
	}),
	z.strictObject({ type: z.literal('desktop-background-window-requested') }),
]);

export type DesktopBackgroundStatusRendererMessage = z.infer<
	typeof DesktopBackgroundStatusRendererMessageSchema
>;
export type DesktopBackgroundStatusSummary = z.infer<typeof DesktopBackgroundStatusSummarySchema>;
