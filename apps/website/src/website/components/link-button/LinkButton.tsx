import type { ReactElement, ReactNode } from 'react';

import { joinClassNames } from '../../joinClassNames';
import { Icon, type IconName } from '../icon/Icon';
import styles from './LinkButton.module.css';

// A link drawn as a solid button, compact as `small`.
export function LinkButton({
	href,
	children,
	icon,
	size = 'regular',
}: {
	href: string;
	children: ReactNode;
	icon?: IconName;
	size?: 'regular' | 'small';
}): ReactElement {
	return (
		<a
			className={joinClassNames(styles['button'], size === 'small' && styles['small'])}
			href={href}
		>
			{icon === undefined ? null : <Icon name={icon} />}
			<span>{children}</span>
		</a>
	);
}
