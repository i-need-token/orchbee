import { describe, expect, it } from 'vitest';

import { ORCHBEE_LOCALES } from '../../locale';
import { getOrchbeeMessages } from '../getOrchbeeMessages';

function collectVariables(message: string): ReadonlySet<string> {
	const variables = new Set<string>();
	for (const match of message.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\s*(?:,|\})/gu)) {
		const variable = match[1];
		if (variable !== undefined) variables.add(variable);
	}
	return variables;
}

describe('getOrchbeeMessages', () => {
	it('keeps locale IDs and variables aligned with the English source catalog', () => {
		const sourceMessages = getOrchbeeMessages('en-US');
		const sourceIds = new Set(Object.keys(sourceMessages));

		for (const locale of ORCHBEE_LOCALES) {
			const messages = getOrchbeeMessages(locale);
			expect(new Set(Object.keys(messages))).toEqual(sourceIds);

			for (const [messageId, sourceMessage] of Object.entries(sourceMessages)) {
				const message = messages[messageId];
				if (message === undefined) throw new Error(`Missing ${locale}: ${messageId}`);
				expect(collectVariables(message)).toEqual(collectVariables(sourceMessage));
			}
		}
	});
});
