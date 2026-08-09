export type ServerFallbackRoute =
	| 'api-not-found'
	| 'document'
	| 'not-found'
	| 'static-not-found'
	| 'websocket-upgrade-required';

export interface ServerRequestRouteInput {
	readonly acceptsHtml: boolean;
	readonly method: string;
	readonly pathname: string;
}

export function classifyServerRoute(input: ServerRequestRouteInput): ServerFallbackRoute {
	if (isNamespacePath(input.pathname, '/api')) return 'api-not-found';
	if (isNamespacePath(input.pathname, '/ws')) return 'websocket-upgrade-required';
	if (isNamespacePath(input.pathname, '/static')) return 'static-not-found';
	const method = input.method.toUpperCase();
	if ((method === 'GET' || method === 'HEAD') && input.acceptsHtml) return 'document';
	return 'not-found';
}

function isNamespacePath(pathname: string, namespace: string): boolean {
	return pathname === namespace || pathname.startsWith(`${namespace}/`);
}
