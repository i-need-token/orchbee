import { useOrchbeeIntl, websiteShellMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { Icon } from '../icon/Icon';
import styles from './CopyButton.module.css';

// Copies `text`. It stays hidden until activation finds a clipboard to write to, and it is marked
// data-copied for a moment after a copy.
export function CopyButton({ text }: { text: string }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<button
			className={styles['copy']}
			type='button'
			data-copy={text}
			aria-label={t(messages.copyCommand)}
			hidden
		>
			<Icon name='copy' />
		</button>
	);
}
