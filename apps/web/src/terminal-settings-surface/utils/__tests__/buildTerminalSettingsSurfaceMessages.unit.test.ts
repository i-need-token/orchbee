import { createOrchbeeIntl } from '@orchbee/i18n';
import { describe, expect, it } from 'vitest';

import { buildTerminalSettingsSurfaceMessages } from '../buildTerminalSettingsSurfaceMessages';

describe('buildTerminalSettingsSurfaceMessages', () => {
	it('builds complete localized labels and interpolated validation copy', () => {
		const english = createOrchbeeIntl('en-US');
		const chinese = createOrchbeeIntl('zh-CN');

		expect(
			buildTerminalSettingsSurfaceMessages((descriptor, values) =>
				english.formatMessage(descriptor, values),
			),
		).toMatchObject({
			copyOnSelectLabel: 'Copy on select',
			scrollbackBytesInvalid: 'Enter a whole number from 0 to 1,000,000,000.',
			terminalPaddingInvalid: 'Enter a whole number from 0 to 32.',
		});
		expect(
			buildTerminalSettingsSurfaceMessages((descriptor, values) =>
				chinese.formatMessage(descriptor, values),
			).copyOnSelectLabel,
		).toBe('选中时复制');
	});
});
