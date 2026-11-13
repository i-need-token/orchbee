import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import styles from './CopyButton.module.css';

// Hidden in the static page; the page script shows it and copies `text` to the clipboard.
export function CopyButton({ text }: { text: string }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<button
			className={styles['copy']}
			type='button'
			aria-label={t(messages.copyCommand)}
			data-copy={text}
			data-copied-label={t(messages.copied)}
			hidden
		>
			{t(messages.copy)}
		</button>
	);
}
