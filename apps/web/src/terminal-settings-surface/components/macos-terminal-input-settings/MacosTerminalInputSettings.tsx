import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { isTerminalOptionAsAltMode } from '@orchbee/terminal';
import { NativeSelect, NativeSelectOption } from '@orchbee/ui/components';
import * as React from 'react';

import {
	type MacosTerminalInputSettingsController,
	useMacosTerminalInputSettings,
} from '../../../macos-terminal-input-settings';
import { EmptyResults } from '../empty-results/EmptyResults';
import { SettingsSection } from '../terminal-settings-sections/components/settings-section/SettingsSection';
import { TerminalSettingsRow } from '../terminal-settings-sections/components/terminal-settings-row/TerminalSettingsRow';
import styles from './MacosTerminalInputSettings.module.css';

interface MacosTerminalInputSettingsProps {
	readonly controller: MacosTerminalInputSettingsController;
	readonly query: string;
}

export function MacosTerminalInputSettings({
	controller,
	query,
}: MacosTerminalInputSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const snapshot = useMacosTerminalInputSettings(controller);
	const messages = {
		both: intl.formatMessage(terminalSettingsMessages.macosOptionAsAltBoth),
		description: intl.formatMessage(terminalSettingsMessages.macosOptionAsAltDescription),
		label: intl.formatMessage(terminalSettingsMessages.macosOptionAsAltLabel),
		left: intl.formatMessage(terminalSettingsMessages.macosOptionAsAltLeft),
		noResults: intl.formatMessage(terminalSettingsMessages.noResults),
		off: intl.formatMessage(terminalSettingsMessages.macosOptionAsAltOff),
		persistenceInvalid: intl.formatMessage(terminalSettingsMessages.persistenceInvalidWarning),
		persistenceRead: intl.formatMessage(terminalSettingsMessages.persistenceReadWarning),
		persistenceWrite: intl.formatMessage(terminalSettingsMessages.persistenceWriteWarning),
		right: intl.formatMessage(terminalSettingsMessages.macosOptionAsAltRight),
		section: intl.formatMessage(terminalSettingsMessages.macosOptionKeySection),
	};
	const normalizedQuery = query.trim().toLocaleLowerCase(intl.locale);
	const visible =
		normalizedQuery.length === 0 ||
		Object.values(messages).some((value) =>
			value.toLocaleLowerCase(intl.locale).includes(normalizedQuery),
		);
	if (!visible) return <EmptyResults message={messages.noResults} />;

	let persistenceWarning: string | null = null;
	switch (snapshot.persistenceIssue) {
		case 'invalid-record':
			persistenceWarning = messages.persistenceInvalid;
			break;
		case 'read-failed':
			persistenceWarning = messages.persistenceRead;
			break;
		case 'write-failed':
			persistenceWarning = messages.persistenceWrite;
			break;
	}

	return (
		<div className={styles['root']}>
			{persistenceWarning === null ? null : (
				<div className={styles['warning']} role='alert'>
					{persistenceWarning}
				</div>
			)}
			<SettingsSection title={messages.section} visible>
				<TerminalSettingsRow description={messages.description} label={messages.label} visible>
					<NativeSelect
						aria-label={messages.label}
						className={styles['control']}
						value={snapshot.preferences.optionAsAlt}
						onChange={(event): void => {
							const mode = event.currentTarget.value;
							if (isTerminalOptionAsAltMode(mode)) controller.update({ optionAsAlt: mode });
						}}
					>
						<NativeSelectOption value='off'>{messages.off}</NativeSelectOption>
						<NativeSelectOption value='left'>{messages.left}</NativeSelectOption>
						<NativeSelectOption value='right'>{messages.right}</NativeSelectOption>
						<NativeSelectOption value='both'>{messages.both}</NativeSelectOption>
					</NativeSelect>
				</TerminalSettingsRow>
			</SettingsSection>
		</div>
	);
}
