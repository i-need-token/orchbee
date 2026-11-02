import { z } from 'zod';

import {
	AGENT_INTEGRATION_IDS,
	AgentIntegrationIdSchema,
	type AgentIntegrationId,
} from './agent-integration-management-control';

const AGENT_INTEGRATION_REMINDER_STORAGE_KEY = 'orchbee:web:agent-integration-reminders:v1';

export type AgentIntegrationReminderPersistenceIssue =
	| 'invalid-record'
	| 'read-failed'
	| 'write-failed';

export interface AgentIntegrationReminderStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

export interface AgentIntegrationReminderPreferences {
	readonly dismissedIntegrationIds: readonly AgentIntegrationId[];
}

const DEFAULT_AGENT_INTEGRATION_REMINDER_PREFERENCES: AgentIntegrationReminderPreferences =
	Object.freeze({ dismissedIntegrationIds: Object.freeze([]) });

const storedAgentIntegrationReminderPreferencesSchema = z.strictObject({
	dismissedIntegrationIds: z
		.array(AgentIntegrationIdSchema)
		.max(AGENT_INTEGRATION_IDS.length)
		.refine((ids) => new Set(ids).size === ids.length, 'Integration IDs must be unique'),
});

export function readAgentIntegrationReminderPreferences(storage: AgentIntegrationReminderStorage): {
	readonly issue: Exclude<AgentIntegrationReminderPersistenceIssue, 'write-failed'> | null;
	readonly preferences: AgentIntegrationReminderPreferences;
} {
	let serialized: string | null;
	try {
		serialized = storage.getItem(AGENT_INTEGRATION_REMINDER_STORAGE_KEY);
	} catch {
		return {
			issue: 'read-failed',
			preferences: DEFAULT_AGENT_INTEGRATION_REMINDER_PREFERENCES,
		};
	}
	if (serialized === null) {
		return { issue: null, preferences: DEFAULT_AGENT_INTEGRATION_REMINDER_PREFERENCES };
	}
	try {
		const result = storedAgentIntegrationReminderPreferencesSchema.safeParse(
			JSON.parse(serialized),
		);
		return result.success
			? {
					issue: null,
					preferences: Object.freeze({
						dismissedIntegrationIds: canonicalizeIntegrationIds(
							result.data.dismissedIntegrationIds,
						),
					}),
				}
			: {
					issue: 'invalid-record',
					preferences: DEFAULT_AGENT_INTEGRATION_REMINDER_PREFERENCES,
				};
	} catch {
		return {
			issue: 'invalid-record',
			preferences: DEFAULT_AGENT_INTEGRATION_REMINDER_PREFERENCES,
		};
	}
}

export function writeAgentIntegrationReminderPreferences(
	storage: AgentIntegrationReminderStorage,
	preferences: AgentIntegrationReminderPreferences,
): boolean {
	try {
		storage.setItem(
			AGENT_INTEGRATION_REMINDER_STORAGE_KEY,
			JSON.stringify({ dismissedIntegrationIds: preferences.dismissedIntegrationIds }),
		);
		return true;
	} catch {
		return false;
	}
}

export function canonicalizeIntegrationIds(
	ids: ReadonlySet<AgentIntegrationId> | readonly AgentIntegrationId[],
): readonly AgentIntegrationId[] {
	const idSet = new Set(ids);
	return Object.freeze(AGENT_INTEGRATION_IDS.filter((integrationId) => idSet.has(integrationId)));
}
