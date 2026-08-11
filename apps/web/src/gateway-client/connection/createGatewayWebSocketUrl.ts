const GATEWAY_BROWSER_HOSTNAME = /^(?:\d{1,3}\.){3}\d{1,3}$/u;

export function createGatewayWebSocketUrl(
	hostname: string,
	port: number,
	pathname: string,
): string {
	if (!isIPv4(hostname)) {
		throw new Error('The Gateway Browser endpoint requires an IPv4 page hostname.');
	}
	if (!Number.isInteger(port) || port < 1 || port > 65_535 || !pathname.startsWith('/')) {
		throw new Error('The Gateway Browser endpoint is invalid.');
	}
	const url = new URL('ws://127.0.0.1');
	url.hostname = hostname;
	url.port = String(port);
	url.pathname = pathname;
	return url.href;
}

function isIPv4(value: string): boolean {
	if (!GATEWAY_BROWSER_HOSTNAME.test(value)) return false;
	const octets = value.split('.');
	return octets.every((octet) => {
		if (!/^(?:0|[1-9]\d{0,2})$/u.test(octet)) return false;
		const number = Number(octet);
		return number >= 0 && number <= 255;
	});
}
