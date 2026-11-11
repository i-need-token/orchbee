import { useMemoizedFn } from '@orchbee/foundation/react';
import type {
	WorkbenchHostActionCallback,
	WorkbenchTerminalFontSizeChangeCallback,
} from '../../workbench';
import { useLayoutEffect, useRef, type RefObject } from 'react';

import {
	type TerminalSettingsController,
	type TerminalSettingsLanding,
	useTerminalSettings,
} from '../../terminal-settings';

interface UseWebApplicationOptions {
	/** The active terminal's input, which leaving Settings returns to when the Workbench has one. */
	readonly findActiveTerminal?: () => HTMLElement | null;
	readonly terminalSettingsController: TerminalSettingsController;
}

interface WebApplicationView {
	readonly applicationReference: RefObject<HTMLDivElement | null>;
	readonly closeSettings: () => void;
	readonly onSettings: WorkbenchHostActionCallback;
	readonly onShortcuts: WorkbenchHostActionCallback;
	readonly onTerminalFontSizeChange: WorkbenchTerminalFontSizeChangeCallback;
	readonly settingsOpen: boolean;
	readonly workbenchReference: RefObject<HTMLDivElement | null>;
}

export function useWebApplication({
	findActiveTerminal,
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
			const terminal = findActiveTerminal?.() ?? null;
			if (terminal?.isConnected === true) {
				terminal.focus();
				return;
			}
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
		if (result.kind === 'changed') {
			restoreWorkbenchFocus();
		}
	});

	const openSettings = (
		context: Parameters<WorkbenchHostActionCallback>[0],
		landing: TerminalSettingsLanding | null,
	): ReturnType<WorkbenchHostActionCallback> => {
		if (context.signal.aborted) return { kind: 'cancelled' };
		if (terminalSettingsController.getSnapshot().open) return { kind: 'unchanged' };
		const activeElement = applicationReference.current?.ownerDocument.activeElement;
		returnFocusReference.current =
			activeElement instanceof HTMLElement && activeElement.isConnected ? activeElement : null;
		context.markCommitted();
		terminalSettingsController.open(landing);
		return { kind: 'completed' };
	};
	const onSettings: WorkbenchHostActionCallback = useMemoizedFn((context) =>
		openSettings(context, null),
	);
	// The shortcut reference is the Settings section that lists every key.
	const onShortcuts: WorkbenchHostActionCallback = useMemoizedFn((context) =>
		openSettings(context, 'keyboard-shortcuts'),
	);
	const onTerminalFontSizeChange: WorkbenchTerminalFontSizeChangeCallback = useMemoizedFn(
		(context, change) => {
			if (context.signal.aborted) return { kind: 'cancelled' };
			const result = terminalSettingsController.changeFontSize(change);
			switch (result.kind) {
				// The settings file applies the size; every terminal follows its answer.
				case 'pending':
					context.markCommitted();
					return { kind: 'completed' };
				case 'unchanged':
					return { kind: 'unchanged' };
				case 'disposed':
					return { code: 'terminal-settings-unavailable', kind: 'blocked' };
				case 'invalid':
					return {
						failure: { code: 'terminal-font-size-invalid' },
						kind: 'failed',
					};
			}
		},
	);

	return {
		applicationReference,
		closeSettings,
		onSettings,
		onShortcuts,
		onTerminalFontSizeChange,
		settingsOpen: settings.open,
		workbenchReference,
	};
}
