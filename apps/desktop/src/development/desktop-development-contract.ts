import { z } from 'zod';

/** Asks a development Main to quit so that its runtime can start the rebuilt one. */
export const DesktopDevelopmentRestartRequestSchema = z.strictObject({
	generation: z.number().int().positive(),
	type: z.literal('orchbee-desktop-development-restart-request'),
});

export type DesktopDevelopmentRestartRequest = z.infer<
	typeof DesktopDevelopmentRestartRequestSchema
>;
