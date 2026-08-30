import { type FormEvent, type RefObject, useRef, useState, useSyncExternalStore } from 'react';

import type {
	DesktopSshProjectEnrollmentDialogController,
	DesktopSshProjectEnrollmentDialogState,
} from '../model/DesktopSshProjectEnrollmentController';

interface DesktopSshProjectEnrollmentDialogView {
	readonly close: () => void;
	readonly destinationInputReference: RefObject<HTMLInputElement | null>;
	readonly destinationInvalid: boolean;
	readonly state: DesktopSshProjectEnrollmentDialogState;
	readonly submitDestination: (event: FormEvent<HTMLFormElement>) => void;
}

export function useDesktopSshProjectEnrollmentDialog(
	controller: DesktopSshProjectEnrollmentDialogController,
): DesktopSshProjectEnrollmentDialogView {
	const state = useSyncExternalStore(
		controller.subscribeDialog,
		controller.getDialogSnapshot,
		controller.getDialogSnapshot,
	);
	const destinationInputReference = useRef<HTMLInputElement>(null);
	const [destinationSubmissionAttempted, setDestinationSubmissionAttempted] = useState(false);
	const editingDestination =
		(state.kind === 'attachment' || state.kind === 'enrollment') && state.phase === 'editing';
	const destinationInvalid =
		editingDestination && destinationSubmissionAttempted && !state.canSubmit;

	return {
		close: () => {
			setDestinationSubmissionAttempted(false);
			controller.close();
		},
		destinationInputReference,
		destinationInvalid,
		state,
		submitDestination: (event) => {
			event.preventDefault();
			if (!editingDestination) return;
			setDestinationSubmissionAttempted(true);
			if (!state.canSubmit) {
				destinationInputReference.current?.focus();
				return;
			}
			setDestinationSubmissionAttempted(false);
			controller.submit();
		},
	};
}
