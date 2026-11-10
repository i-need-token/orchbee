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

/** The identity Main chose for Quit All's Session stop, which `server stop-status` reads it under. */
const DesktopSessionStopIdSchema = z.string().regex(/^(?!0{32}$)[0-9a-f]{32}$/u);

/**
 * What became of the Session stop the page was asked for: `recorded` when the Server holds it and
 * runs it once; `unknown` when the request may have reached the Server, so the stop's status
 * decides; `not-sent` when the page has no Server connection that could carry it. `busy`,
 * `unmanaged`, and `refused` recorded nothing.
 */
const DesktopSessionStopAnswerSchema = z.enum([
	'busy',
	'not-sent',
	'recorded',
	'refused',
	'unknown',
	'unmanaged',
]);

/**
 * What the page sends Main over the background-status port: the summary whenever it changes, a
 * request to show the window, which a page cannot do for a window Main hid, and its answer to a
 * Session stop.
 */
export const DesktopBackgroundStatusRendererMessageSchema = z.discriminatedUnion('type', [
	z.strictObject({
		summary: DesktopBackgroundStatusSummarySchema,
		type: z.literal('desktop-background-status-changed'),
	}),
	z.strictObject({ type: z.literal('desktop-background-window-requested') }),
	z.strictObject({
		answer: DesktopSessionStopAnswerSchema,
		stopId: DesktopSessionStopIdSchema,
		type: z.literal('desktop-session-stop-answered'),
	}),
]);

/**
 * What Main sends the page over the same port: Quit All's Session stop, which only the page's
 * Server connection can ask for. The page answers each one once.
 */
export const DesktopBackgroundStatusMainMessageSchema = z.discriminatedUnion('type', [
	z.strictObject({
		stopId: DesktopSessionStopIdSchema,
		type: z.literal('desktop-session-stop-requested'),
	}),
]);

export type DesktopBackgroundStatusMainMessage = z.infer<
	typeof DesktopBackgroundStatusMainMessageSchema
>;
export type DesktopBackgroundStatusRendererMessage = z.infer<
	typeof DesktopBackgroundStatusRendererMessageSchema
>;
export type DesktopBackgroundStatusSummary = z.infer<typeof DesktopBackgroundStatusSummarySchema>;
export type DesktopSessionStopAnswer = z.infer<typeof DesktopSessionStopAnswerSchema>;
