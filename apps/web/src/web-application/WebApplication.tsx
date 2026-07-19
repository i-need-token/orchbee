import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import {
	Workbench,
	type WorkbenchLayoutStorage,
	type WorkbenchShortcutPlatform,
	type WorkbenchTab,
} from '@orchbee/workbench';
import * as React from 'react';

import { useWebApplicationTabs } from './hooks/useWebApplicationTabs';
import styles from './WebApplication.module.css';

export interface WebApplicationProps {
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly locale: OrchbeeLocale;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly tabs: readonly WorkbenchTab[];
}

export function WebApplication({
	layoutStorage,
	locale,
	shortcutPlatform,
	tabs,
}: WebApplicationProps): React.ReactNode {
	const workspace = useWebApplicationTabs(locale, tabs);

	return (
		<OrchbeeI18nProvider locale={locale}>
			<TooltipProvider>
				<div className={styles['root']}>
					<Workbench
						activeTabId={workspace.activeTabId}
						layoutStorage={layoutStorage}
						onActiveTabChange={workspace.onActiveTabChange}
						onTabClose={workspace.onTabClose}
						onTabCreate={workspace.onTabCreate}
						onTabMove={workspace.onTabMove}
						shortcutPlatform={shortcutPlatform}
						tabs={workspace.tabs}
					/>
				</div>
			</TooltipProvider>
		</OrchbeeI18nProvider>
	);
}
