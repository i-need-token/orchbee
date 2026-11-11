import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { NativeSelect, NativeSelectOption } from '@orchbee/ui/components';
import type * as React from 'react';

import {
	type ApplicationLocaleController,
	useApplicationLocale,
} from '../../../application-locale';
import type { UserPreferencesControl } from '../../../user-preferences';
import { useSharedPreferenceDefault } from '../../hooks/useSharedPreferenceDefault';
import { SettingsRow } from '../settings-row/SettingsRow';

interface ApplicationLanguageSettingsProps {
	readonly controller: ApplicationLocaleController;
	readonly query: string;
	readonly userPreferences: UserPreferencesControl;
}

/** The language row: follow the system, or pin English or Simplified Chinese. */
export function ApplicationLanguageSettings({
	controller,
	query,
	userPreferences,
}: ApplicationLanguageSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const snapshot = useApplicationLocale(controller);
	const locale = useSharedPreferenceDefault(userPreferences, 'locale');
	const label = intl.formatMessage(terminalSettingsMessages.pageLanguageLabel);
	return (
		<SettingsRow
			description={intl.formatMessage(terminalSettingsMessages.pageLanguageDescription)}
			label={label}
			modified={locale.modified}
			query={query}
			searchTerms={['English', '中文', 'language', '语言']}
			settingId='general.language'
			onRestore={locale.restore}
		>
			<NativeSelect
				aria-label={label}
				size='sm'
				value={snapshot.preference ?? 'system'}
				onChange={(event): void => {
					controller.select(event.currentTarget.value);
				}}
			>
				<NativeSelectOption value='system'>
					{intl.formatMessage(terminalSettingsMessages.sharedSystem)}
				</NativeSelectOption>
				<NativeSelectOption value='en-US'>
					{intl.formatMessage(terminalSettingsMessages.languageEnglish)}
				</NativeSelectOption>
				<NativeSelectOption value='zh-CN'>
					{intl.formatMessage(terminalSettingsMessages.languageSimplifiedChinese)}
				</NativeSelectOption>
			</NativeSelect>
		</SettingsRow>
	);
}
