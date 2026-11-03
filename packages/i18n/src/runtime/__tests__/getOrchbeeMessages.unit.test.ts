import { describe, expect, it } from 'vitest';

import { ORCHBEE_LOCALES } from '../../locale';
import { getOrchbeeMessages } from '../getOrchbeeMessages';

/**
 * Collects ICU argument names. A plural or select branch body is message text, so the `Close` in
 * `=1 {Close}` is a word, not an argument.
 */
function collectVariables(message: string): ReadonlySet<string> {
	const variables = new Set<string>();
	let index = 0;
	const consume = (pattern: RegExp): RegExpExecArray | null => {
		pattern.lastIndex = index;
		const match = pattern.exec(message);
		if (match !== null) index = pattern.lastIndex;
		return match;
	};
	// Reads text up to the brace that closes the enclosing branch, or to the end.
	const readMessage = (): void => {
		while (index < message.length && message[index] !== '}') {
			const quoted = message[index] === "'" && /[{}]/u.test(message[index + 1] ?? '');
			if (message[index] === '{') {
				index += 1;
				readArgument();
			} else if (quoted) {
				const close = message.indexOf("'", index + 1);
				index = close === -1 ? message.length : close + 1;
			} else {
				index += 1;
			}
		}
	};
	const readArgument = (): void => {
		const head = consume(/\s*([A-Za-z]\w*)\s*(?:,\s*([a-z]+)\s*)?/uy);
		const name = head?.[1];
		if (name !== undefined) variables.add(name);
		const type = head?.[2];
		if (type === 'plural' || type === 'select' || type === 'selectordinal') {
			consume(/,\s*(?:offset:\s*\d+\s*)?/uy);
			while (consume(/\s*[^\s{}]+\s*\{/uy) !== null) {
				readMessage();
				index += 1;
			}
			consume(/\s*/uy);
		} else {
			// A formatted argument's style runs to its closing brace.
			while (index < message.length && message[index] !== '}') index += 1;
		}
		index += 1;
	};
	readMessage();
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
