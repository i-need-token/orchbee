import {
	UserPreferencesRequestSchema,
	UserPreferencesResponseSchema,
} from '@orchbee/workbench-client-protocol';
import { z } from 'zod';

export const DesktopUserPreferencesCommandSchema = z.strictObject({
	type: z.literal('desktop-user-preferences-requested'),
	requestId: z.number().int().positive().max(Number.MAX_SAFE_INTEGER),
	request: UserPreferencesRequestSchema,
});

export const DesktopUserPreferencesEventSchema = z.strictObject({
	type: z.literal('desktop-user-preferences-observed'),
	requestId: z.number().int().positive().max(Number.MAX_SAFE_INTEGER).nullable(),
	response: UserPreferencesResponseSchema,
	applicationIssue: z.enum(['host-rejected']).nullable(),
});

export type DesktopUserPreferencesEvent = z.infer<typeof DesktopUserPreferencesEventSchema>;
