import { sourceControlMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ReactNode } from 'react';

import { useSourceControlCount } from '../../../source-control';
import type { FileBarToggleBadgeProps } from '../../file-bar-contract';
import { FileBarCountBadge } from '../file-bar-count-badge/FileBarCountBadge';

/**
 * Source Control's count on the File Bar's own button while the column is hidden, and the
 * description the button points to, which says what the badge counts.
 */
export function FileBarToggleBadge({ descriptionId, session }: FileBarToggleBadgeProps): ReactNode {
	const intl = useOrchbeeIntl();
	const count = useSourceControlCount(session.git);
	if (count === null || count === 0) return null;
	return (
		<>
			<FileBarCountBadge count={count} />
			<span hidden id={descriptionId}>
				{intl.formatMessage(sourceControlMessages.badge, { count })}
			</span>
		</>
	);
}
