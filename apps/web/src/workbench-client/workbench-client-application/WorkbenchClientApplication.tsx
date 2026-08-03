import type { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import {
	type WorkbenchLayoutStorage,
	type WorkbenchProjectWorkspaceRuntime,
	type WorkbenchShortcutPlatform,
} from '@orchbee/workbench';
import type { ReactNode } from 'react';

import { WebApplication } from '../../web-application';
import {
	ProjectedTerminalSurface,
	ProjectedTerminalSurfaceCapabilityContext,
	type ProjectedTerminalSurfaceCapability,
} from '../projected-terminal-surface';

export interface WorkbenchClientApplicationProps {
	readonly actionRuntime: ActionRuntime;
	readonly children?: ReactNode;
	readonly layoutStorage: WorkbenchLayoutStorage;
	readonly locale: OrchbeeLocale;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly terminalSurfaceCapability: ProjectedTerminalSurfaceCapability;
}

export function WorkbenchClientApplication({
	actionRuntime,
	children,
	layoutStorage,
	locale,
	projectWorkspaceRuntime,
	shortcutPlatform,
	terminalSurfaceCapability,
}: WorkbenchClientApplicationProps): ReactNode {
	return (
		<ProjectedTerminalSurfaceCapabilityContext.Provider value={terminalSurfaceCapability}>
			<OrchbeeI18nProvider locale={locale}>
				<TooltipProvider>
					<>
						<WebApplication
							actionRuntime={actionRuntime}
							layoutStorage={layoutStorage}
							projectWorkspaceRuntime={projectWorkspaceRuntime}
							shortcutPlatform={shortcutPlatform}
							terminalSurface={ProjectedTerminalSurface}
						/>
						{children}
					</>
				</TooltipProvider>
			</OrchbeeI18nProvider>
		</ProjectedTerminalSurfaceCapabilityContext.Provider>
	);
}
