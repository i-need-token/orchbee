import { useMemoizedFn } from '@orchbee/foundation/react';
import type { WorkbenchHostActionCallback } from '@orchbee/workbench';
import { useLayoutEffect, useRef, type RefObject } from 'react';

import { type TerminalSettingsController, useTerminalSettings } from '../../terminal-settings';

interface UseWebApplicationOptions {
	readonly terminalSettingsController: TerminalSettingsController;
}

interface WebApplicationView {
	readonly applicationReference: RefObject<HTMLDivElement | null>;
	readonly closeSettings: () => void;
	readonly onSettings: WorkbenchHostActionCallback;
	readonly settingsOpen: boolean;
	readonly workbenchReference: RefObject<HTMLDivElement | null>;
}

export function useWebApplication({
	terminalSettingsController,
}: UseWebApplicationOptions): WebApplicationView {
	const settings = useTerminalSettings(terminalSettingsController);
	const applicationReference = useRef<HTMLDivElement>(null);
	const workbenchReference = useRef<HTMLDivElement>(null);
	const returnFocusReference = useRef<HTMLElement | null>(null);

	useLayoutEffect(() => {
		const workbench = workbenchReference.current;
		if (workbench === null) return;
		workbench.inert = settings.open;
		if (settings.open) workbench.setAttribute('aria-hidden', 'true');
		else workbench.removeAttribute('aria-hidden');
		return () => {
			workbench.inert = false;
			workbench.removeAttribute('aria-hidden');
		};
	}, [settings.open]);

	const restoreWorkbenchFocus = useMemoizedFn((): void => {
		const application = applicationReference.current;
		if (application === null) return;
		const captured = returnFocusReference.current;
		returnFocusReference.current = null;
		queueMicrotask(() => {
			if (captured?.isConnected === true) {
				captured.focus();
				return;
			}
			const settingsAction = application.querySelector<HTMLElement>(
				"[data-slot='workbench-sidebar-settings']",
			);
			if (settingsAction?.isConnected === true) {
				settingsAction.focus();
				return;
			}
			const workbench = workbenchReference.current;
			if (workbench !== null) {
				workbench.tabIndex = -1;
				workbench.focus();
			}
		});
	});

	const closeSettings = useMemoizedFn((): void => {
		const result = terminalSettingsController.close();
		if (result.kind === 'changed') restoreWorkbenchFocus();
	});

	const onSettings: WorkbenchHostActionCallback = useMemoizedFn(
		(context: Parameters<WorkbenchHostActionCallback>[0]) => {
			if (context.signal.aborted) return { kind: 'cancelled' };
			if (terminalSettingsController.getSnapshot().open) return { kind: 'unchanged' };
			const activeElement = applicationReference.current?.ownerDocument.activeElement;
			returnFocusReference.current =
				activeElement instanceof HTMLElement && activeElement.isConnected ? activeElement : null;
			context.markCommitted();
			terminalSettingsController.open();
			return { kind: 'completed' };
		},
	);

	return {
		applicationReference,
		closeSettings,
		onSettings,
		settingsOpen: settings.open,
		workbenchReference,
	};
}
