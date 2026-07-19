import { describe, expect, it } from 'vitest';

import { webMessages, workbenchMessages } from '../../descriptors';
import { ORCHBEE_LOCALES } from '../../locale';
import { getOrchbeeMessages } from '../getOrchbeeMessages';

const descriptors = [...Object.values(workbenchMessages), ...Object.values(webMessages)];

function collectVariables(message: string): ReadonlySet<string> {
	const variables = new Set<string>();
	for (const match of message.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\s*(?:,|\})/gu)) {
		const variable = match[1];
		if (variable !== undefined) variables.add(variable);
	}
	return variables;
}

describe('getOrchbeeMessages', () => {
	it('keeps every locale catalog complete', () => {
		const descriptorIds = new Set(descriptors.map((descriptor) => descriptor.id));

		for (const locale of ORCHBEE_LOCALES) {
			const catalogIds = new Set(Object.keys(getOrchbeeMessages(locale)));
			expect(catalogIds).toEqual(descriptorIds);
		}
	});

	it('uses the English locale catalog as every descriptor default', () => {
		const sourceMessages = getOrchbeeMessages('en-US');
		for (const descriptor of descriptors) {
			const sourceMessage = sourceMessages[descriptor.id];
			if (sourceMessage === undefined) throw new Error(`Missing source: ${descriptor.id}`);
			expect(descriptor.defaultMessage).toBe(sourceMessage);
		}
	});

	it('keeps variables compatible across every locale', () => {
		for (const locale of ORCHBEE_LOCALES) {
			const messages = getOrchbeeMessages(locale);
			for (const descriptor of descriptors) {
				const message = messages[descriptor.id];
				if (message === undefined) throw new Error(`Missing ${locale}: ${descriptor.id}`);
				expect(collectVariables(message)).toEqual(collectVariables(descriptor.defaultMessage));
			}
		}
	});
});
