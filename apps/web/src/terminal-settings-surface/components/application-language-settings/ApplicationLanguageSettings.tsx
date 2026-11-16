import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type * as React from 'react';

import {
	type ApplicationLocaleController,
	useApplicationLocale,
} from '../../../application-locale';
import type { UserSettingsControl } from '../../../user-settings';
import { useSetting } from '../../hooks/useSetting';
import { SettingFailure } from '../setting-failure/SettingFailure';
import { SettingsRow } from '../settings-row/SettingsRow';
import { SettingsSelect } from '../settings-select/SettingsSelect';

interface ApplicationLanguageSettingsProps {
	readonly controller: ApplicationLocaleController;
	readonly query: string;
	readonly userSettings: UserSettingsControl;
}

/** The language row: follow the system, or pin English or Simplified Chinese. */
export function ApplicationLanguageSettings({
	controller,
	query,
	userSettings,
}: ApplicationLanguageSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const snapshot = useApplicationLocale(controller);
	const locale = useSetting(userSettings, 'general.language');
	const label = intl.formatMessage(terminalSettingsMessages.pageLanguageLabel);
	return (
		<SettingsRow
			description={intl.formatMessage(terminalSettingsMessages.pageLanguageDescription)}
			label={label}
			modified={locale.modified}
			query={query}
			searchTerms={['English', '中文', 'language', '语言']}
			settingId='general.language'
			status={<SettingFailure message={locale.failureMessage} />}
			onRestore={locale.readOnly ? undefined : locale.restore}
		>
			<SettingsSelect
				disabled={locale.readOnly}
				label={label}
				options={[
					{ label: intl.formatMessage(terminalSettingsMessages.sharedSystem), value: 'system' },
					{ label: intl.formatMessage(terminalSettingsMessages.languageEnglish), value: 'en-US' },
					{
						label: intl.formatMessage(terminalSettingsMessages.languageSimplifiedChinese),
						value: 'zh-CN',
					},
				]}
				value={snapshot.preference ?? 'system'}
				onValueChange={(value): void => {
					controller.select(value);
				}}
			/>
		</SettingsRow>
	);
}
