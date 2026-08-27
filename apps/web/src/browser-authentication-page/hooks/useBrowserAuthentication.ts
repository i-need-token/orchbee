import { useSyncExternalStore } from 'react';

import type {
	BrowserAuthenticationControllerPort,
	BrowserAuthenticationState,
} from '../model/browser-authentication-controller';

export function useBrowserAuthentication(
	controller: BrowserAuthenticationControllerPort,
): BrowserAuthenticationState {
	return useSyncExternalStore(controller.subscribe, controller.getSnapshot, controller.getSnapshot);
}
