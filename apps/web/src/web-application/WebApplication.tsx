import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { Workbench, type WorkbenchLayoutStorage } from '@orchbee/workbench';
import * as React from 'react';

import styles from './WebApplication.module.css';

export interface WebApplicationProps {
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly locale: OrchbeeLocale;
}

export function WebApplication({ layoutStorage, locale }: WebApplicationProps): React.ReactNode {
	return (
		<OrchbeeI18nProvider locale={locale}>
			<div className={styles['root']}>
				<Workbench layoutStorage={layoutStorage} />
			</div>
		</OrchbeeI18nProvider>
	);
}
