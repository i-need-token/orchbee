import { useSyncExternalStore } from 'react';

import type { ApplicationLocaleSnapshot, ApplicationLocaleSource } from '../model';

export function useApplicationLocale(source: ApplicationLocaleSource): ApplicationLocaleSnapshot {
	return useSyncExternalStore(source.subscribe, source.getSnapshot, source.getSnapshot);
}
