import {
	createRoute,
	createRouter,
	notFound,
	redirect,
	type RouterHistory,
} from '@tanstack/react-router';

import type { WebApplicationProps } from '../web-application';
import { WorkbenchRoute } from './components/workbench-route/WorkbenchRoute';
import { WorkbenchRouteCoordinator } from './WorkbenchRouteCoordinator';
import {
	workbenchRoutePath,
	type WorkbenchRouteRequest,
	type WorkbenchRouteResolution,
} from './workbench-route-model';
import {
	webApplicationRootRoute,
	type WebApplicationRouteContext,
} from './web-application-root-route';

const indexRoute = createRoute({
	beforeLoad: ({ context }) => prepareRoute(context, { kind: 'root' }),
	component: WorkbenchRoute,
	getParentRoute: () => webApplicationRootRoute,
	path: '/',
});

const standaloneRoute = createRoute({
	beforeLoad: ({ context }) => prepareRoute(context, { kind: 'standalone' }),
	component: WorkbenchRoute,
	getParentRoute: () => webApplicationRootRoute,
	path: '/terminal',
});

const standaloneTabsRoute = createRoute({
	beforeLoad: ({ context }) => prepareRoute(context, { kind: 'standalone' }),
	component: WorkbenchRoute,
	getParentRoute: () => webApplicationRootRoute,
	path: '/terminal/tabs',
});

const standaloneTabRoute = createRoute({
	beforeLoad: ({ context, params }) =>
		prepareRoute(context, { kind: 'standalone', tabId: params.tabId }),
	component: WorkbenchRoute,
	getParentRoute: () => webApplicationRootRoute,
	path: '/terminal/tabs/$tabId',
});

const projectsRoute = createRoute({
	beforeLoad: ({ context }) => prepareRoute(context, { kind: 'projects' }),
	component: WorkbenchRoute,
	getParentRoute: () => webApplicationRootRoute,
	path: '/projects',
});

const projectRoute = createRoute({
	beforeLoad: ({ context, params }) =>
		prepareRoute(context, { kind: 'project', projectId: params.projectId }),
	component: WorkbenchRoute,
	getParentRoute: () => webApplicationRootRoute,
	path: '/projects/$projectId',
});

const projectWorkspacesRoute = createRoute({
	beforeLoad: ({ context, params }) =>
		prepareRoute(context, { kind: 'project', projectId: params.projectId }),
	component: WorkbenchRoute,
	getParentRoute: () => webApplicationRootRoute,
	path: '/projects/$projectId/workspaces',
});

const workspaceRoute = createRoute({
	beforeLoad: ({ context, params }) =>
		prepareRoute(context, {
			kind: 'workspace',
			projectId: params.projectId,
			workspaceId: params.workspaceId,
		}),
	component: WorkbenchRoute,
	getParentRoute: () => webApplicationRootRoute,
	path: '/projects/$projectId/workspaces/$workspaceId',
});

const workspaceTabsRoute = createRoute({
	beforeLoad: ({ context, params }) =>
		prepareRoute(context, {
			kind: 'workspace',
			projectId: params.projectId,
			workspaceId: params.workspaceId,
		}),
	component: WorkbenchRoute,
	getParentRoute: () => webApplicationRootRoute,
	path: '/projects/$projectId/workspaces/$workspaceId/tabs',
});

const workspaceTabRoute = createRoute({
	beforeLoad: ({ context, params }) =>
		prepareRoute(context, {
			kind: 'workspace',
			projectId: params.projectId,
			tabId: params.tabId,
			workspaceId: params.workspaceId,
		}),
	component: WorkbenchRoute,
	getParentRoute: () => webApplicationRootRoute,
	path: '/projects/$projectId/workspaces/$workspaceId/tabs/$tabId',
});

const routeTree = webApplicationRootRoute.addChildren([
	indexRoute,
	standaloneRoute,
	standaloneTabsRoute,
	standaloneTabRoute,
	projectsRoute,
	projectRoute,
	projectWorkspacesRoute,
	workspaceRoute,
	workspaceTabsRoute,
	workspaceTabRoute,
]);

interface WebApplicationRoutingInput extends Omit<WebApplicationProps, 'navigation'> {
	readonly getInitialTerminalTitle: () => string;
}

const createWebApplicationRoutingImplementation = (
	input: WebApplicationRoutingInput,
	history?: RouterHistory | undefined,
) => {
	const { getInitialTerminalTitle, ...application } = input;
	let routeHistory: RouterHistory | undefined = history;
	const coordinator = new WorkbenchRouteCoordinator(
		application.projectWorkspaceRuntime,
		(location, options) => {
			if (routeHistory === undefined) throw new Error('The application Router is unavailable.');
			const path = workbenchRoutePath(location);
			if (options.replace) routeHistory.replace(path);
			else routeHistory.push(path);
		},
		getInitialTerminalTitle,
	);
	const router = createRouter({
		context: { application, coordinator },
		...(history === undefined ? {} : { history }),
		isServer: false,
		routeTree,
		trailingSlash: 'never',
	});
	routeHistory = router.history;
	let disposed = false;
	return {
		coordinator,
		dispose: (): void => {
			if (disposed) return;
			disposed = true;
			coordinator.dispose();
			router.history.destroy();
		},
		router,
	};
};

export type WebApplicationRouting = ReturnType<typeof createWebApplicationRoutingImplementation>;
export type WebApplicationRouter = WebApplicationRouting['router'];

export function createWebApplicationRouting(
	input: WebApplicationRoutingInput,
	history?: RouterHistory | undefined,
): WebApplicationRouting {
	return createWebApplicationRoutingImplementation(input, history);
}

function prepareRoute(context: WebApplicationRouteContext, request: WorkbenchRouteRequest): void {
	const resolution = context.coordinator.prepare(request);
	handleRouteResolution(resolution);
}

function handleRouteResolution(resolution: WorkbenchRouteResolution): void {
	if (resolution.kind === 'redirect') {
		throw redirect({ replace: true, to: workbenchRoutePath(resolution.location) });
	}
	if (resolution.kind === 'unavailable') {
		throw notFound({ data: { reason: resolution.reason } });
	}
}
