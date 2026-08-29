import { TooltipProvider } from '@orchbee/ui/components';
import { RouterProvider } from '@tanstack/react-router';
import type { ReactNode } from 'react';

import type { WebApplicationRouter } from '../../application-routing';
import type { ApplicationLocaleSource } from '../../application-locale';
import { ApplicationLocaleProvider } from '../../application-locale-provider';
import {
	ProjectedTerminalSurfaceCapabilityContext,
	type ProjectedTerminalSurfaceCapability,
} from '../projected-terminal-surface';

export interface WorkbenchClientApplicationProps {
	readonly applicationLocaleSource: ApplicationLocaleSource;
	readonly children?: ReactNode;
	readonly router: WebApplicationRouter;
	readonly terminalSurfaceCapability: ProjectedTerminalSurfaceCapability;
}

export function WorkbenchClientApplication({
	applicationLocaleSource,
	children,
	router,
	terminalSurfaceCapability,
}: WorkbenchClientApplicationProps): ReactNode {
	return (
		<ProjectedTerminalSurfaceCapabilityContext.Provider value={terminalSurfaceCapability}>
			<ApplicationLocaleProvider source={applicationLocaleSource}>
				<TooltipProvider>
					<>
						<RouterProvider router={router} />
						{children}
					</>
				</TooltipProvider>
			</ApplicationLocaleProvider>
		</ProjectedTerminalSurfaceCapabilityContext.Provider>
	);
}
