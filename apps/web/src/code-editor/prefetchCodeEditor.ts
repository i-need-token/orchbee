import type { OrchbeeLocale } from '@orchbee/i18n';

import { loadCodeEditor } from './loadCodeEditor';

/**
 * Loads the code editor when the page is next idle, so the first file a person opens does not wait
 * for Monaco. Returns a function that cancels a prefetch not yet started.
 */
export function prefetchCodeEditor(locale: OrchbeeLocale): () => void {
	const prefetch = (): void => {
		// A failed prefetch stays silent; the load that needs the editor reports its own failure.
		loadCodeEditor(locale).catch(() => undefined);
	};
	if (typeof requestIdleCallback === 'function') {
		const handle = requestIdleCallback(prefetch);
		return () => cancelIdleCallback(handle);
	}
	const handle = setTimeout(prefetch, 0);
	return () => clearTimeout(handle);
}
