import { createRootRouteWithContext } from '@tanstack/react-router';

import { RouteUnavailable } from '../route-unavailable';
import type { WebApplicationProps } from '../web-application';
import { WebApplicationRouteOutlet } from './components/web-application-route-outlet/WebApplicationRouteOutlet';
import type { WorkbenchRouteCoordinator } from './WorkbenchRouteCoordinator';

export interface WebApplicationRouteContext {
	readonly application: Omit<WebApplicationProps, 'navigation'>;
	readonly coordinator: WorkbenchRouteCoordinator;
}

export const webApplicationRootRoute = createRootRouteWithContext<WebApplicationRouteContext>()({
	component: WebApplicationRouteOutlet,
	notFoundComponent: RouteUnavailable,
});
