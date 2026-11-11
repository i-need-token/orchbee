import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import styles from './SettingsBanner.module.css';

interface SettingsBannerProps {
	/** The banner's one action, such as allowing notifications. */
	readonly action?: React.ReactNode;
	readonly description?: React.ReactNode;
	readonly onDismiss?: (() => void) | undefined;
	readonly title: string;
}

/** A page-wide notice above the category title, dismissible when it is not a blocking problem. */
export function SettingsBanner({
	action,
	description,
	onDismiss,
	title,
}: SettingsBannerProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	return (
		<div className={styles['root']} data-slot='settings-banner' role='status'>
			<div className={styles['copy']}>
				<div className={styles['title']}>{title}</div>
				{description === undefined ? null : (
					<div className={styles['description']}>{description}</div>
				)}
			</div>
			{action === undefined ? null : <div className={styles['action']}>{action}</div>}
			{onDismiss === undefined ? null : (
				<button
					aria-label={intl.formatMessage(terminalSettingsMessages.pageDismiss)}
					className={styles['dismiss']}
					title={intl.formatMessage(terminalSettingsMessages.pageDismiss)}
					type='button'
					onClick={onDismiss}
				>
					<ProductIcon name='close' />
				</button>
			)}
		</div>
	);
}
