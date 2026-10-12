import {
	UserPreferencesRequestSchema,
	UserPreferencesResponseSchema,
} from '@orchbee/workbench-client-protocol';
import { z } from 'zod';

export const DesktopUserPreferencesCommandSchema = z.strictObject({
	type: z.literal('desktop-user-preferences-requested'),
	requestId: z.number().int().positive().max(Number.MAX_SAFE_INTEGER),
	request: UserPreferencesRequestSchema.refine(
		(request) => request.operation !== 'import-desktop' || request.part === 'renderer',
		'The renderer may only import its own legacy preferences',
	),
});

export const DesktopUserPreferencesEventSchema = z.strictObject({
	type: z.literal('desktop-user-preferences-observed'),
	requestId: z.number().int().positive().max(Number.MAX_SAFE_INTEGER).nullable(),
	response: UserPreferencesResponseSchema,
	applicationIssue: z.enum(['host-rejected']).nullable(),
	legacyIssue: z.boolean().default(false),
});

export type DesktopUserPreferencesCommand = z.infer<typeof DesktopUserPreferencesCommandSchema>;
export type DesktopUserPreferencesEvent = z.infer<typeof DesktopUserPreferencesEventSchema>;
