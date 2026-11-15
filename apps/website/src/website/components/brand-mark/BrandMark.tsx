import type { ReactElement } from 'react';

import styles from './BrandMark.module.css';

// The Orchbee mark followed by the name, linking to the home page.
export function BrandMark({ href }: { href: string }): ReactElement {
	return (
		<a className={styles['brand']} href={href}>
			<img className={styles['logo']} src='/icon.svg' alt='' width='26' height='26' />
			Orchbee
		</a>
	);
}
