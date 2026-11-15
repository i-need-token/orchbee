import type { ReactElement } from 'react';

import styles from './HexDot.module.css';

// A small honey hexagon that marks a line of text as the Session or the product speaking.
export function HexDot(): ReactElement {
	return <i className={styles['dot']} aria-hidden='true' />;
}
