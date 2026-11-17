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
import { WorkbenchRenderBoundary } from '../workbench-render-boundary';

export interface WorkbenchClientApplicationProps {
	readonly applicationLocaleSource: ApplicationLocaleSource;
	readonly children?: ReactNode;
	/** What takes the Workbench's place when it fails to render; `children` stay beside it. */
	readonly failure: ReactNode;
	readonly router: WebApplicationRouter;
	readonly terminalSurfaceCapability: ProjectedTerminalSurfaceCapability;
}

export function WorkbenchClientApplication({
	applicationLocaleSource,
	children,
	failure,
	router,
	terminalSurfaceCapability,
}: WorkbenchClientApplicationProps): ReactNode {
	return (
		<ProjectedTerminalSurfaceCapabilityContext.Provider value={terminalSurfaceCapability}>
			<ApplicationLocaleProvider source={applicationLocaleSource}>
				<TooltipProvider>
					<>
						<WorkbenchRenderBoundary fallback={() => failure} site='workbench-render'>
							<RouterProvider router={router} />
						</WorkbenchRenderBoundary>
						{children}
					</>
				</TooltipProvider>
			</ApplicationLocaleProvider>
		</ProjectedTerminalSurfaceCapabilityContext.Provider>
	);
}
