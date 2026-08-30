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
			trimTrailingBlankRowsForAgentsLabel: 'Trim trailing blank rows for CLI agents',
		});
		expect(
			buildTerminalSettingsSurfaceMessages((descriptor, values) =>
				chinese.formatMessage(descriptor, values),
			),
		).toMatchObject({
			copyOnSelectLabel: '选中时复制',
			trimTrailingBlankRowsForAgentsLabel: '裁剪 CLI Agent 尾部空白行',
		});
	});
});
