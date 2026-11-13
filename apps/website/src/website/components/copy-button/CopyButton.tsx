import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { Icon } from '../icon/Icon';
import styles from './CopyButton.module.css';

// Copies `text`. The page script shows it once the clipboard is available and marks it
// `data-copied` for a moment after a copy.
export function CopyButton({
	text,
	showLabel = false,
	tone = 'page',
}: {
	text: string;
	showLabel?: boolean;
	tone?: 'page' | 'terminal';
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<button
			className={styles[tone]}
			type='button'
			data-copy={text}
			aria-label={t(messages.copyCommand)}
			hidden
		>
			<span className={styles['idle']}>
				<Icon name='copy' size={15} />
				{showLabel ? <span>{t(messages.copy)}</span> : null}
			</span>
			<span className={styles['done']}>
				<Icon name='ok' size={15} />
				<span>{t(messages.copied)}</span>
			</span>
		</button>
	);
}
