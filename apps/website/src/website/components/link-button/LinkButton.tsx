import type { ReactElement, ReactNode } from 'react';

import styles from './LinkButton.module.css';

// A pill-shaped link: honey for the page's main call to action, outlined for the alternative.
export function LinkButton({
	href,
	variant = 'primary',
	children,
}: {
	href: string;
	variant?: 'primary' | 'secondary';
	children: ReactNode;
}): ReactElement {
	return (
		<a className={styles[variant]} href={href}>
			{children}
		</a>
	);
}
