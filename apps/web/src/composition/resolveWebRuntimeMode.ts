type WebRuntimeMode = 'browser' | 'desktop' | 'gateway' | 'invalid';

export function resolveWebRuntimeMode(search: string): WebRuntimeMode {
	const values = new URLSearchParams(search).getAll('orchbee-runtime');
	if (values.length === 0) return 'browser';
	if (values.length !== 1) return 'invalid';
	return values[0] === 'desktop' || values[0] === 'gateway' ? values[0] : 'invalid';
}
