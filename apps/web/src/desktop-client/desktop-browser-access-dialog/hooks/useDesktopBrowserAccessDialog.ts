import { useSyncExternalStore } from 'react';

import type {
	DesktopBrowserAccessDialogController,
	DesktopBrowserAccessDialogState,
} from '../model/DesktopBrowserAccessController';

export function useDesktopBrowserAccessDialog(
	controller: DesktopBrowserAccessDialogController,
): DesktopBrowserAccessDialogState {
	return useSyncExternalStore(controller.subscribe, controller.getSnapshot, controller.getSnapshot);
}
