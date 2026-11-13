import type { ReactElement } from 'react';

import styles from './BrandMark.module.css';

// The Orchbee mark from the brand kit (resources/icon.svg, published as /icon.svg), filling the
// box its parent sizes. It always sits beside the name or inside a labelled link, so it carries
// no text of its own.
export function BrandMark(): ReactElement {
	return <img className={styles['mark']} src='/icon.svg' alt='' width='1024' height='1024' />;
}
