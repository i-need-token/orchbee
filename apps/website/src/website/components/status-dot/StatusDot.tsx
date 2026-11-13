import type { ReactElement } from 'react';

import styles from './StatusDot.module.css';

export type StatusDotState = 'attention' | 'working' | 'done' | 'failed' | 'idle' | 'unread';

// An Agent status light outside the terminal pictures, shaped and colored like the product's
// glyph. The page script changes `data-state`; `hook` names the dot for it.
export function StatusDot({
	state,
	hook,
	place = 'inline',
}: {
	state: StatusDotState;
	hook?: string;
	place?: 'inline' | 'badge' | 'text';
}): ReactElement {
	return <i className={styles[place]} data-state={state} data-hook={hook} />;
}
