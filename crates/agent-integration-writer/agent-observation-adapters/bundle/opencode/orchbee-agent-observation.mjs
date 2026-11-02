import {
	createOpenCodeObservation,
	sanitizeOpenCodeFamilyEvent,
} from '../scripts/opencode-family-observation.mjs';

export const OrchbeeAgentObservation = async () => createOpenCodeObservation('opencode');

export function sanitizeOpenCodeEvent(value) {
	return sanitizeOpenCodeFamilyEvent('opencode', value);
}
