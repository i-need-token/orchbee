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
	DesktopApplicationLifetimeDialog,
	type DesktopApplicationLifetimeDialogController,
} from '../desktop-application-lifetime-dialog';
import {
	DesktopTerminalSurface,
	DesktopTerminalSurfaceCapabilityContext,
	type DesktopTerminalSurfaceCapability,
} from '../desktop-terminal-surface';

export interface DesktopWorkbenchProps {
	readonly actionRuntime: ActionRuntime;
	readonly applicationLifetimeCoordinator: DesktopApplicationLifetimeDialogController;
	readonly layoutStorage: WorkbenchLayoutStorage;
	readonly locale: OrchbeeLocale;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly terminalSurfaceCapability: DesktopTerminalSurfaceCapability;
}

export function DesktopWorkbench({
	actionRuntime,
	applicationLifetimeCoordinator,
	layoutStorage,
	locale,
	projectWorkspaceRuntime,
	shortcutPlatform,
	terminalSurfaceCapability,
}: DesktopWorkbenchProps): ReactNode {
	return (
		<DesktopTerminalSurfaceCapabilityContext.Provider value={terminalSurfaceCapability}>
			<OrchbeeI18nProvider locale={locale}>
				<TooltipProvider>
					<>
						<WebApplication
							actionRuntime={actionRuntime}
							layoutStorage={layoutStorage}
							projectWorkspaceRuntime={projectWorkspaceRuntime}
							shortcutPlatform={shortcutPlatform}
							terminalSurface={DesktopTerminalSurface}
						/>
						<DesktopApplicationLifetimeDialog controller={applicationLifetimeCoordinator} />
					</>
				</TooltipProvider>
			</OrchbeeI18nProvider>
		</DesktopTerminalSurfaceCapabilityContext.Provider>
	);
}
