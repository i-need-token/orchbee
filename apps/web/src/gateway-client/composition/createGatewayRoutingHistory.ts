import {
	encodeGatewayBrowserSearch,
	type GatewayBrowserConnection,
} from '@orchbee/gateway-protocol';
import { createBrowserHistory, type RouterHistory } from '@tanstack/react-router';

const GATEWAY_RUNTIME_KEY = 'orchbee-runtime';

export function createGatewayRoutingHistory(
	targetWindow: Window,
	connection: GatewayBrowserConnection,
): RouterHistory {
	const gatewayParameters = new URLSearchParams();
	gatewayParameters.set(GATEWAY_RUNTIME_KEY, 'gateway');
	for (const [key, value] of new URLSearchParams(encodeGatewayBrowserSearch(connection))) {
		gatewayParameters.set(key, value);
	}
	return createBrowserHistory({
		createHref: (path) => mergeGatewayParameters(path, gatewayParameters),
		window: targetWindow,
	});
}

function mergeGatewayParameters(path: string, gatewayParameters: URLSearchParams): string {
	const hashIndex = path.indexOf('#');
	const hash = hashIndex === -1 ? '' : path.slice(hashIndex);
	const pathBeforeHash = hashIndex === -1 ? path : path.slice(0, hashIndex);
	const searchIndex = pathBeforeHash.indexOf('?');
	const pathname = searchIndex === -1 ? pathBeforeHash : pathBeforeHash.slice(0, searchIndex);
	const parameters = new URLSearchParams(
		searchIndex === -1 ? '' : pathBeforeHash.slice(searchIndex + 1),
	);
	for (const [key, value] of gatewayParameters) parameters.set(key, value);
	return `${pathname}?${parameters.toString()}${hash}`;
}
