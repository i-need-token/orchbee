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
import { useWebApplicationProjectWorkspace } from './hooks/useWebApplicationProjectWorkspace';

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
	const projectWorkspaceRuntime = useWebApplicationProjectWorkspace(locale);

	return (
		<OrchbeeI18nProvider locale={locale}>
			<TooltipProvider>
				<div className={styles['root']}>
					<Workbench
						actionRuntime={actionRuntime}
						layoutStorage={layoutStorage}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform={shortcutPlatform}
					/>
				</div>
			</TooltipProvider>
		</OrchbeeI18nProvider>
	);
}
