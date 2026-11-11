import * as React from 'react';

const QUERY = '(prefers-color-scheme: dark)';

/** Whether the operating system asks for dark appearance; false where the host cannot tell. */
export function useSystemPrefersDark(): boolean {
	return React.useSyncExternalStore(subscribe, readPrefersDark, readPrefersDark);
}

function subscribe(listener: () => void): () => void {
	const media = mediaQuery();
	media?.addEventListener('change', listener);
	return () => media?.removeEventListener('change', listener);
}

function readPrefersDark(): boolean {
	return mediaQuery()?.matches === true;
}

function mediaQuery(): MediaQueryList | null {
	return typeof globalThis.matchMedia === 'function' ? globalThis.matchMedia(QUERY) : null;
}
