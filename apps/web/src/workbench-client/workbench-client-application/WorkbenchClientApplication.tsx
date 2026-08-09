import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import { RouterProvider } from '@tanstack/react-router';
import type { ReactNode } from 'react';

import type { WebApplicationRouter } from '../../application-routing';
import {
	ProjectedTerminalSurfaceCapabilityContext,
	type ProjectedTerminalSurfaceCapability,
} from '../projected-terminal-surface';

export interface WorkbenchClientApplicationProps {
	readonly children?: ReactNode;
	readonly locale: OrchbeeLocale;
	readonly router: WebApplicationRouter;
	readonly terminalSurfaceCapability: ProjectedTerminalSurfaceCapability;
}

export function WorkbenchClientApplication({
	children,
	locale,
	router,
	terminalSurfaceCapability,
}: WorkbenchClientApplicationProps): ReactNode {
	return (
		<ProjectedTerminalSurfaceCapabilityContext.Provider value={terminalSurfaceCapability}>
			<OrchbeeI18nProvider locale={locale}>
				<TooltipProvider>
					<>
						<RouterProvider router={router} />
						{children}
					</>
				</TooltipProvider>
			</OrchbeeI18nProvider>
		</ProjectedTerminalSurfaceCapabilityContext.Provider>
	);
}
