type WebRuntimeMode = 'browser' | 'desktop' | 'invalid';

export function resolveWebRuntimeMode(search: string): WebRuntimeMode {
	const values = new URLSearchParams(search).getAll('orchbee-runtime');
	if (values.length === 0) return 'browser';
	return values.length === 1 && values[0] === 'desktop' ? 'desktop' : 'invalid';
}
