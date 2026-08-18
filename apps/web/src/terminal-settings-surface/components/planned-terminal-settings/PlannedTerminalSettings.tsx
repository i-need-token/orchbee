import { terminalSettingsCatalogMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Badge, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { PlannedTerminalSettingSectionView } from '../../types/terminal-settings-surface';
import { SettingsSection } from '../settings-section/SettingsSection';
import { TerminalSettingsRow } from '../terminal-settings-row/TerminalSettingsRow';
import { PlannedControl } from './components/planned-control/PlannedControl';
import styles from './PlannedTerminalSettings.module.css';

interface PlannedTerminalSettingsProps {
	readonly notice: string;
	readonly sections: readonly PlannedTerminalSettingSectionView[];
}

export function PlannedTerminalSettings({
	notice,
	sections,
}: PlannedTerminalSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	if (sections.length === 0) return null;
	const defaultValue = intl.formatMessage(
		terminalSettingsCatalogMessages.get('terminal.settings.catalog.common.defaultValue'),
	);
	const manage = intl.formatMessage(
		terminalSettingsCatalogMessages.get('terminal.settings.catalog.common.manage'),
	);
	const planned = intl.formatMessage(
		terminalSettingsCatalogMessages.get('terminal.settings.catalog.common.planned'),
	);

	return (
		<div className={styles['root']} data-slot='planned-terminal-settings'>
			<div className={styles['notice']} role='note'>
				<ProductIcon name='action' />
				<span>{notice}</span>
			</div>
			{sections.map((section) => (
				<SettingsSection key={section.id} title={section.title} visible>
					{section.items.map((item) => (
						<TerminalSettingsRow
							description={item.description}
							key={item.id}
							label={item.label}
							settingId={item.id}
							status={
								<Badge className={styles['badge']} variant='secondary'>
									{planned}
								</Badge>
							}
							visible
						>
							<div className={styles['control']} data-planned-setting={item.id}>
								<PlannedControl
									defaultValue={defaultValue}
									kind={item.control}
									label={item.label}
									manage={manage}
								/>
							</div>
						</TerminalSettingsRow>
					))}
				</SettingsSection>
			))}
		</div>
	);
}
