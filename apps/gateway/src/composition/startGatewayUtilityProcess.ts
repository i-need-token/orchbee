import { LocalDevelopmentBrowserGateway, type GatewayParentPort } from '../browser-gateway';

export function startGatewayUtilityProcess(
	parentPort: GatewayParentPort | undefined = process.parentPort,
): LocalDevelopmentBrowserGateway {
	if (parentPort === undefined) {
		throw new Error('The Orchbee Gateway requires an Electron Utility Process parent port.');
	}

	const gateway = new LocalDevelopmentBrowserGateway(parentPort);
	gateway.start();
	return gateway;
}
