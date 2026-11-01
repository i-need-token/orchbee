type WebRuntimeMode = 'browser' | 'desktop' | 'desktop-server-client' | 'invalid';

export interface WebRuntimeEvidence {
	readonly protocol: string;
	readonly search: string;
	readonly userAgent: string;
}

export function resolveWebRuntimeMode({
	protocol,
	search,
	userAgent,
}: WebRuntimeEvidence): WebRuntimeMode {
	const values = new URLSearchParams(search).getAll('orchbee-runtime');
	if (values.length > 0) return 'invalid';
	if (protocol !== 'orchbee:' && !isElectronUserAgent(userAgent)) return 'browser';
	return new URLSearchParams(search).has('orchbee-server-client')
		? 'desktop-server-client'
		: 'desktop';
}

function isElectronUserAgent(userAgent: string): boolean {
	return /(?:^|\s)Electron\/\d+\.\d+\.\d+(?:\s|$)/u.test(userAgent);
}
