import {
	openCodeClientPlugin,
	openCodeFamilyPlugin,
	sanitizeOpenCodeFamilyEvent,
} from '../scripts/opencode-family-observation.mjs';

export const orchbeeAgentObservationPlugin = openCodeFamilyPlugin('opencode');

export const orchbeeAgentObservationClientPlugin = openCodeClientPlugin();

export function sanitizeOpenCodeEvent(value) {
	return sanitizeOpenCodeFamilyEvent('opencode', value);
}
