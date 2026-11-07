import {
	openCodeFamilyPlugin,
	sanitizeOpenCodeFamilyEvent,
} from '../scripts/opencode-family-observation.mjs';

export const orchbeeAgentObservationPlugin = openCodeFamilyPlugin('opencode');

export function sanitizeOpenCodeEvent(value) {
	return sanitizeOpenCodeFamilyEvent('opencode', value);
}
