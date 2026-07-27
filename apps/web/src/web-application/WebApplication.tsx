import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import {
	Workbench,
	type WorkbenchLayoutStorage,
	type WorkbenchShortcutPlatform,
} from '@orchbee/workbench';
import * as React from 'react';

import styles from './WebApplication.module.css';
import { useWebApplicationActionRuntime } from './hooks/useWebApplicationActionRuntime';
import { useWebApplicationTabs } from './hooks/useWebApplicationTabs';

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
	const actionRuntime = useWebApplicationActionRuntime();
	const { createTerminal, tabModel } = useWebApplicationTabs(locale);

	return (
		<OrchbeeI18nProvider locale={locale}>
			<TooltipProvider>
				<div className={styles['root']}>
					<Workbench
						actionRuntime={actionRuntime}
						layoutStorage={layoutStorage}
						onNewTerminal={createTerminal}
						shortcutPlatform={shortcutPlatform}
						tabModel={tabModel}
					/>
				</div>
			</TooltipProvider>
		</OrchbeeI18nProvider>
	);
}
