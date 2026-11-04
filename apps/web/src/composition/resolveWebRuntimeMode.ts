type WebRuntimeMode = 'desktop' | 'unsupported';

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
	if (values.length > 0) return 'unsupported';
	return protocol === 'orchbee:' || isElectronUserAgent(userAgent) ? 'desktop' : 'unsupported';
}

function isElectronUserAgent(userAgent: string): boolean {
	return /(?:^|\s)Electron\/\d+\.\d+\.\d+(?:\s|$)/u.test(userAgent);
}
