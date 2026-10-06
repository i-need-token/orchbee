type WebRuntimeMode = 'browser' | 'desktop' | 'invalid';

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
	return protocol === 'orchbee:' || isElectronUserAgent(userAgent) ? 'desktop' : 'browser';
}

function isElectronUserAgent(userAgent: string): boolean {
	return /(?:^|\s)Electron\/\d+\.\d+\.\d+(?:\s|$)/u.test(userAgent);
}
