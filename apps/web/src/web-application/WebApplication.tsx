import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import {
	Workbench,
	type WorkbenchLayoutStorage,
	type WorkbenchShortcutPlatform,
} from '@orchbee/workbench';
import * as React from 'react';

import styles from './WebApplication.module.css';

export interface WebApplicationProps {
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly locale: OrchbeeLocale;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
}

export function WebApplication({
	layoutStorage,
	locale,
	shortcutPlatform,
}: WebApplicationProps): React.ReactNode {
	return (
		<OrchbeeI18nProvider locale={locale}>
			<TooltipProvider>
				<div className={styles['root']}>
					<Workbench layoutStorage={layoutStorage} shortcutPlatform={shortcutPlatform} />
				</div>
			</TooltipProvider>
		</OrchbeeI18nProvider>
	);
}
