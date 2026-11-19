import type { ReactElement, ReactNode } from 'react';

import { joinClassNames } from '../../joinClassNames';
import { Icon, type IconName } from '../icon/Icon';
import styles from './LinkButton.module.css';

// A link drawn as a button: solid when primary, bordered when secondary, compact as `small`.
// `downloadTrigger` marks the link that opens the download dialog.
export function LinkButton({
	href,
	children,
	icon,
	size = 'regular',
	primary = true,
	platform,
	downloadTrigger = false,
}: {
	href: string;
	children: ReactNode;
	icon?: IconName;
	size?: 'regular' | 'small';
	primary?: boolean;
	platform?: string;
	downloadTrigger?: boolean;
}): ReactElement {
	return (
		<a
			className={joinClassNames(styles['button'], size === 'small' && styles['small'])}
			href={href}
			data-primary={primary ? '' : undefined}
			data-platform={platform}
			data-download-trigger={downloadTrigger ? '' : undefined}
		>
			{icon === undefined ? null : <Icon name={icon} />}
			<span>{children}</span>
		</a>
	);
}
