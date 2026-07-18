import { Workbench, type WorkbenchSessionService } from '@orchbee/workbench';
import * as React from 'react';

import styles from './entry.module.css';

export interface WebApplicationProps {
	readonly sessions: WorkbenchSessionService;
}

export function WebApplication({ sessions }: WebApplicationProps): React.ReactNode {
	return (
		<div className={styles['root']}>
			<Workbench sessions={sessions} />
		</div>
	);
}
