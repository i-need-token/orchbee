import type { ReactElement } from 'react';

import styles from './StatusDot.module.css';

export type StatusDotState = 'attention' | 'working' | 'done' | 'failed' | 'idle' | 'unread';

// An Agent status light in the page's own text, shaped and colored like the product's glyph.
export function StatusDot({ state }: { state: StatusDotState }): ReactElement {
	return <i className={styles['inline']} data-state={state} />;
}
