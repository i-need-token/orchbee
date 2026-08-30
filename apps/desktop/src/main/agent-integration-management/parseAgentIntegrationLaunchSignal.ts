import {
	DesktopAgentIntegrationIdSchema,
	type DesktopAgentIntegrationId,
} from '@orchbee/desktop-client-protocol';
import { z } from 'zod';

const AgentIntegrationLaunchSignalSchema = z.strictObject({
	integrationId: DesktopAgentIntegrationIdSchema,
	version: z.literal(3),
});

export function parseAgentIntegrationLaunchSignal(
	payload: string,
): DesktopAgentIntegrationId | null {
	let value: unknown;
	try {
		value = JSON.parse(payload);
	} catch {
		return null;
	}
	const parsed = AgentIntegrationLaunchSignalSchema.safeParse(value);
	return parsed.success ? parsed.data.integrationId : null;
}
