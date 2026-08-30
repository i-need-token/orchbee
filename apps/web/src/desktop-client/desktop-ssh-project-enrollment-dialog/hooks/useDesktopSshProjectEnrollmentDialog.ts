import {
	type FormEvent,
	type RefObject,
	useMemo,
	useRef,
	useState,
	useSyncExternalStore,
} from 'react';

import type {
	DesktopSshProjectEnrollmentDialogController,
	DesktopSshProjectEnrollmentDialogState,
} from '../model/DesktopSshProjectEnrollmentController';
import { desktopSshProjectDestinationOptions } from '../utils/desktop-ssh-project-enrollment-dialog-view';

interface DesktopSshProjectEnrollmentDialogView {
	readonly cancel: () => void;
	readonly close: () => void;
	readonly connectDestination: (destination: string) => void;
	readonly destinationOptions: readonly string[];
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
	const destinationCatalog = useSyncExternalStore(
		controller.subscribeDestinationCatalog,
		controller.getDestinationCatalogSnapshot,
		controller.getDestinationCatalogSnapshot,
	);
	const editingDestination =
		(state.kind === 'attachment' || state.kind === 'enrollment') && state.phase === 'editing';
	const destinationQuery = editingDestination ? state.form.destination : '';
	const destinationOptions = useMemo(
		() => desktopSshProjectDestinationOptions(destinationCatalog, destinationQuery),
		[destinationCatalog, destinationQuery],
	);
	const destinationInputReference = useRef<HTMLInputElement>(null);
	const [destinationSubmissionAttempted, setDestinationSubmissionAttempted] = useState(false);
	const destinationInvalid =
		editingDestination && destinationSubmissionAttempted && !state.canSubmit;

	return {
		cancel: () => {
			setDestinationSubmissionAttempted(false);
			controller.cancel();
		},
		close: () => {
			setDestinationSubmissionAttempted(false);
			controller.close();
		},
		connectDestination: (destination) => {
			if (!editingDestination) return;
			setDestinationSubmissionAttempted(false);
			controller.updateForm({ destination });
			controller.submit();
		},
		destinationOptions,
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
