import { fileBarMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ReactNode } from 'react';

import styles from './FileBarCountBadge.module.css';

interface FileBarCountBadgeProps {
	/** What the badge counts; it shows nothing for none. */
	readonly count: number;
}

/**
 * VS Code's compact count badge over the bottom end of a toggle's icon. Above 999 it counts whole
 * thousands, with a plus while there are more. Assistive technology reads the count from the
 * toggle's name or description instead.
 */
export function FileBarCountBadge({ count }: FileBarCountBadgeProps): ReactNode {
	const intl = useOrchbeeIntl();
	if (count <= 0) return null;
	let text = intl.formatNumber(count);
	if (count > 999) {
		const thousands = Math.floor(count / 1000);
		text = intl.formatMessage(
			count % 1000 === 0 ? fileBarMessages.badgeThousands : fileBarMessages.badgeOverThousands,
			{ count: thousands },
		);
	}
	return (
		<span aria-hidden='true' className={styles['root']} data-slot='file-bar-count-badge'>
			{text}
		</span>
	);
}
