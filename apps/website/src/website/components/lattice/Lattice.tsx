import type { ReactElement } from 'react';

import styles from './Lattice.module.css';

// The faint hexagon lattice behind the top of every page. Activation draws it; without script the
// page simply has no lattice.
export function Lattice(): ReactElement {
	return <canvas className={styles['lattice']} aria-hidden='true' data-lattice='' />;
}
