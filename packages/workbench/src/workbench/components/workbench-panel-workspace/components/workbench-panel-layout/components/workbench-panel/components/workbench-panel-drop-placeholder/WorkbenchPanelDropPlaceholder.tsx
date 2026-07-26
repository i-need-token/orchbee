import * as React from 'react';

import styles from './WorkbenchPanelDropPlaceholder.module.css';

export function WorkbenchPanelDropPlaceholder(): React.ReactNode {
	return (
		<div
			aria-hidden='true'
			className={styles['root']}
			data-slot='workbench-panel-drop-placeholder'
		/>
	);
}
