import { describe, expect, it } from 'vitest';

import { webMessages, workbenchMessages } from '../../descriptors';
import { ORCHBEE_LOCALES } from '../../locale';
import { createOrchbeeIntl } from '../createOrchbeeIntl';

const descriptors = [...Object.values(workbenchMessages), ...Object.values(webMessages)];
const representativeValues = {
	columnCount: 80,
	command: 'version',
	rowCount: 24,
	sessionCount: 2,
	sessionNumber: 3,
	sessionTitle: 'Local shell',
};

describe('createOrchbeeIntl', () => {
	it('formats every product descriptor in every shipped locale', () => {
		for (const locale of ORCHBEE_LOCALES) {
			const intl = createOrchbeeIntl(locale);
			for (const descriptor of descriptors) {
				expect(intl.formatMessage(descriptor, representativeValues).length).toBeGreaterThan(0);
			}
		}
	});

	it('formats locale-specific plural and number output', () => {
		const english = createOrchbeeIntl('en-US');
		const chinese = createOrchbeeIntl('zh-CN');

		expect(english.formatMessage(workbenchMessages.sessionCount, { sessionCount: 1 })).toBe(
			'1 session',
		);
		expect(english.formatMessage(workbenchMessages.sessionCount, { sessionCount: 2 })).toBe(
			'2 sessions',
		);
		expect(chinese.formatMessage(workbenchMessages.sessionCount, { sessionCount: 2 })).toBe(
			'2 个会话',
		);
	});
});
