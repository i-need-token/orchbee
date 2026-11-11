import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type * as React from 'react';

import styles from './SettingsListFilterEmpty.module.css';

interface SettingsListFilterEmptyProps {
	readonly message?: string | undefined;
}

/** The line a filtered list shows when no entry matches. */
export function SettingsListFilterEmpty({
	message,
}: SettingsListFilterEmptyProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	return (
		<div className={styles['root']} data-settings-row>
			{message ?? intl.formatMessage(terminalSettingsMessages.pageFilterNoMatches)}
		</div>
	);
}
