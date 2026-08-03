import { useSyncExternalStore } from 'react';

import type { DesktopApplicationLifetimeState } from '../model/DesktopApplicationLifetimeCoordinator';
import type { DesktopApplicationLifetimeDialogController } from '../types/desktop-application-lifetime-dialog';

export function useDesktopApplicationLifetimeDialog(
	controller: DesktopApplicationLifetimeDialogController,
): DesktopApplicationLifetimeState {
	return useSyncExternalStore(controller.subscribe, controller.getSnapshot);
}
