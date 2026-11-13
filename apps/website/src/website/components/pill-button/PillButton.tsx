import type { ReactElement, ReactNode } from 'react';

import { joinClassNames } from '../../joinClassNames';
import { Icon, type IconName } from '../icon/Icon';
import styles from './PillButton.module.css';

const ICON_SIZES = { large: 17, small: 15, nav: 14 } as const;

// The rounded button of the page: honey for the main action, glass for the others. A link when
// it has an address, otherwise a button the page script finds by `action`.
export function PillButton({
	variant,
	size = 'large',
	icon,
	href,
	action,
	label,
	children,
}: {
	variant: 'primary' | 'glass';
	size?: 'large' | 'small' | 'nav';
	icon?: IconName;
	href?: string;
	action?: string;
	label?: string;
	children: ReactNode;
}): ReactElement {
	const className = joinClassNames(styles['button'], styles[variant], styles[size]);
	const content = (
		<>
			{icon === undefined ? null : <Icon name={icon} size={ICON_SIZES[size]} />}
			{children}
		</>
	);
	return href === undefined ? (
		<button className={className} type='button' data-action={action} aria-label={label}>
			{content}
		</button>
	) : (
		<a className={className} href={href} aria-label={label}>
			{content}
		</a>
	);
}
