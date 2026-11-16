import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';
import { createPortal } from 'react-dom';

import { SettingsSurfaceSlotsContext } from '../../contexts/settings-surface-slots';
import styles from './SettingFailure.module.css';

/**
 * Why a setting's last change did not land, or why a draft or an action was refused. It shows on
 * the dialog's notice line over the content's foot, so the row keeps its height; without a dialog
 * it renders where it stands. Dismissing hides that message until it clears.
 */
export function SettingFailure({ message }: { readonly message: string | null }): React.ReactNode {
	const intl = useOrchbeeIntl();
	const host = React.useContext(SettingsSurfaceSlotsContext)?.noticeHost ?? null;
	const [dismissed, setDismissed] = React.useState<string | null>(null);
	if (message === null) {
		if (dismissed !== null) setDismissed(null);
		return null;
	}
	if (message === dismissed) return null;
	const notice = (
		<div className={styles['root']} data-slot='settings-notice' role='alert'>
			<span className={styles['message']}>{message}</span>
			<button
				aria-label={intl.formatMessage(terminalSettingsMessages.pageDismiss)}
				className={styles['dismiss']}
				type='button'
				onClick={() => setDismissed(message)}
			>
				<ProductIcon name='close' />
			</button>
		</div>
	);
	return host === null ? notice : createPortal(notice, host);
}
