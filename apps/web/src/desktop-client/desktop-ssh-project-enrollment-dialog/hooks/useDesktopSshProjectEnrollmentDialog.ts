import { useSyncExternalStore } from 'react';

import type {
	DesktopSshProjectEnrollmentDialogController,
	DesktopSshProjectEnrollmentDialogState,
} from '../model/DesktopSshProjectEnrollmentController';

export function useDesktopSshProjectEnrollmentDialog(
	controller: DesktopSshProjectEnrollmentDialogController,
): DesktopSshProjectEnrollmentDialogState {
	return useSyncExternalStore(
		controller.subscribeDialog,
		controller.getDialogSnapshot,
		controller.getDialogSnapshot,
	);
}
