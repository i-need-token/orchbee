import { z } from 'zod';

export const DESKTOP_DEVELOPMENT_RESTART_PLAN_ENVIRONMENT_KEY =
	'ORCHBEE_DESKTOP_DEVELOPMENT_RESTART_PLAN';

export const DesktopDevelopmentRestartRequestSchema = z.strictObject({
	generation: z.number().int().positive(),
	reason: z.enum(['desktop', 'terminal-host']),
	type: z.literal('orchbee-desktop-development-restart-request'),
});

export type DesktopDevelopmentRestartRequest = z.infer<
	typeof DesktopDevelopmentRestartRequestSchema
>;
