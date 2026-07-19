import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { Workbench, type WorkbenchSessionService } from '@orchbee/workbench';
import * as React from 'react';

import styles from './web-application.module.css';

export interface WebApplicationProps {
	readonly locale: OrchbeeLocale;
	readonly sessions: WorkbenchSessionService;
}

export function WebApplication({ locale, sessions }: WebApplicationProps): React.ReactNode {
	return (
		<OrchbeeI18nProvider locale={locale}>
			<div className={styles['root']}>
				<Workbench sessions={sessions} />
			</div>
		</OrchbeeI18nProvider>
	);
}
