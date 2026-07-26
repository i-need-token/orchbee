import * as React from 'react';

import styles from './WorkbenchPanelFocusScrim.module.css';

export function WorkbenchPanelFocusScrim(): React.ReactNode {
	return (
		<div aria-hidden='true' className={styles['root']} data-slot='workbench-panel-focus-scrim' />
	);
}
