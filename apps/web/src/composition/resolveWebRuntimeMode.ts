import { resolveServerTerminalRequest } from './resolveServerTerminalRequest';

type WebRuntimeMode = 'browser' | 'desktop' | 'desktop-server-terminal' | 'invalid';

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
	return resolveServerTerminalRequest(search) === null ? 'desktop' : 'desktop-server-terminal';
}

function isElectronUserAgent(userAgent: string): boolean {
	return /(?:^|\s)Electron\/\d+\.\d+\.\d+(?:\s|$)/u.test(userAgent);
}
