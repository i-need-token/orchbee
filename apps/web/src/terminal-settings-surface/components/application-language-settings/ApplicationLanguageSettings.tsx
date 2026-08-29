import {
	terminalSettingsCatalogMessages,
	terminalSettingsMessages,
	useOrchbeeIntl,
} from '@orchbee/i18n';
import { NativeSelect, NativeSelectOption } from '@orchbee/ui/components';
import type * as React from 'react';

import {
	type ApplicationLocaleController,
	useApplicationLocale,
} from '../../../application-locale';
import { SettingsSection } from '../settings-section/SettingsSection';
import { TerminalSettingsRow } from '../terminal-settings-row/TerminalSettingsRow';
import styles from './ApplicationLanguageSettings.module.css';

interface ApplicationLanguageSettingsProps {
	readonly controller: ApplicationLocaleController;
}

export function ApplicationLanguageSettings({
	controller,
}: ApplicationLanguageSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const snapshot = useApplicationLocale(controller);
	const label = intl.formatMessage(
		terminalSettingsCatalogMessages.get('terminal.settings.catalog.items.general.language.label'),
	);
	return (
		<SettingsSection title={intl.formatMessage(terminalSettingsMessages.languageSection)} visible>
			<TerminalSettingsRow
				description={intl.formatMessage(
					terminalSettingsCatalogMessages.get(
						'terminal.settings.catalog.items.general.language.description',
					),
				)}
				label={label}
				settingId='general.language'
				visible
			>
				<div className={styles['control']}>
					<NativeSelect
						aria-label={label}
						className={styles['select']}
						value={snapshot.effectiveLocale}
						onChange={(event): void => {
							controller.select(event.currentTarget.value);
						}}
					>
						<NativeSelectOption value='en-US'>
							{intl.formatMessage(terminalSettingsMessages.languageEnglish)}
						</NativeSelectOption>
						<NativeSelectOption value='zh-CN'>
							{intl.formatMessage(terminalSettingsMessages.languageSimplifiedChinese)}
						</NativeSelectOption>
					</NativeSelect>
					<span className={styles['notice']}>
						{intl.formatMessage(terminalSettingsMessages.languageAutomaticFallbackNotice)}
					</span>
				</div>
			</TerminalSettingsRow>
		</SettingsSection>
	);
}
