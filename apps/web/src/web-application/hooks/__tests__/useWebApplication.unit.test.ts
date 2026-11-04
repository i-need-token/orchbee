// @vitest-environment jsdom

import { ActionRuntime, defineAction, noActionInput } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import type { WorkbenchTerminalSurfaceProps } from '../../../workbench';
import { act, createElement, useEffect, useLayoutEffect, useRef, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { TerminalSettingsController } from '../../../terminal-settings';
import { ApplicationLocaleController } from '../../../application-locale';
import { KeyboardShortcutSettingsController } from '../../../keyboard-shortcuts';
import {
	createSnapshotRuntime,
	oneWorkspaceSnapshot,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import { WebApplication } from '../../WebApplication';
import { inertWorkbenchAuthority } from '../../../__tests__/inertWorkbenchAuthority';
import { memoryUserPreferences } from '../../../__tests__/memoryUserPreferences';

class TestResizeObserver {
	public disconnect(): void {}
	public observe(_target: Element, _options?: ResizeObserverOptions): void {}
	public unobserve(_target: Element): void {}
}

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
	vi.stubGlobal('ResizeObserver', TestResizeObserver);
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
	vi.unstubAllGlobals();
});

describe('useWebApplication', () => {
	it('routes Workbench shortcuts by the visible product surface and restores terminal focus', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const actionRuntime = new ActionRuntime();
		const projectWorkspaceRuntime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1)]),
		);
		const terminalSettingsController = new TerminalSettingsController({
			storage: { getItem: () => null, setItem: () => undefined },
			themeTarget: document.documentElement,
			userPreferences: memoryUserPreferences(),
		});
		const initialFontSize = terminalSettingsController.getSnapshot().preferences.fontSize;
		const keyboardShortcutSettingsController = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage: { getItem: () => null, setItem: () => undefined },
		});
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(
						TooltipProvider,
						undefined,
						createElement(WebApplication, {
							actionRuntime,
							applicationLocaleController: createApplicationLocaleController(),
							authority: inertAuthority,
							keyboardShortcutSettingsController,
							projectWorkspaceRuntime,
							shortcutPlatform: 'apple',
							terminalSettingsController,
							terminalSurface: FocusTerminalSurface,
						}),
					),
				),
			);
		});

		const terminalInput = requiredElement<HTMLTextAreaElement>(
			container.querySelector('[data-test-terminal-input="true"]'),
		);
		expect(document.activeElement).toBe(terminalInput);
		const increaseFontSize = appleCommand('Equal');
		await act(async () => terminalInput.dispatchEvent(increaseFontSize));
		expect(increaseFontSize.defaultPrevented).toBe(true);
		expect(terminalSettingsController.getSnapshot().preferences.fontSize).toBe(initialFontSize + 2);

		const settingsAction = requiredElement<HTMLButtonElement>(
			container.querySelector("[data-slot='workbench-sidebar-settings']"),
		);
		await act(async () => settingsAction.click());
		const settingsInput = await vi.waitFor(() =>
			requiredElement<HTMLInputElement>(
				container.querySelector("input[aria-label='Search settings']"),
			),
		);
		expect(document.activeElement).toBe(settingsInput);
		const decreaseFontSizeFromSettings = appleCommand('Minus');
		await act(async () => settingsInput.dispatchEvent(decreaseFontSizeFromSettings));
		expect(decreaseFontSizeFromSettings.defaultPrevented).toBe(false);
		expect(terminalSettingsController.getSnapshot().preferences.fontSize).toBe(initialFontSize + 2);
		const createFromSettings = appleCommand('KeyT');
		await act(async () => settingsInput.dispatchEvent(createFromSettings));
		expect(createFromSettings.defaultPrevented).toBe(false);
		expect(
			projectWorkspaceRuntime.getActiveTerminalContext().tabModel.getSnapshot().tabs,
		).toHaveLength(1);
		expect(document.activeElement).toBe(settingsInput);
		await act(async () => {
			settingsInput.dispatchEvent(
				new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Escape' }),
			);
		});
		await vi.waitFor(() =>
			expect(container.querySelector('[data-slot="terminal-settings-surface"]')).toBeNull(),
		);
		await vi.waitFor(() => expect(document.activeElement).toBe(terminalInput));
		const resetFontSize = appleCommand('Digit0');
		await act(async () => terminalInput.dispatchEvent(resetFontSize));
		expect(resetFontSize.defaultPrevented).toBe(true);
		expect(terminalSettingsController.getSnapshot().preferences.fontSize).toBe(initialFontSize);
		await act(async () => root.unmount());
		container.remove();
		keyboardShortcutSettingsController.dispose();
		terminalSettingsController.dispose();
		actionRuntime.dispose();
	}, 15_000);

	it('keeps the Workbench mounted, makes it inert, and restores focus after close', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const actionRuntime = new ActionRuntime();
		const projectWorkspaceRuntime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1)]),
		);
		const terminalSettingsController = new TerminalSettingsController({
			storage: { getItem: () => null, setItem: () => undefined },
			themeTarget: document.documentElement,
			userPreferences: memoryUserPreferences(),
		});
		const keyboardShortcutSettingsController = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage: { getItem: () => null, setItem: () => undefined },
		});
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(
						TooltipProvider,
						undefined,
						createElement(WebApplication, {
							actionRuntime,
							applicationLocaleController: createApplicationLocaleController(),
							authority: inertAuthority,
							keyboardShortcutSettingsController,
							projectWorkspaceRuntime,
							shortcutPlatform: 'apple',
							terminalSettingsController,
							terminalSurface: TestTerminalSurface,
						}),
					),
				),
			);
		});

		const settingsAction = requiredElement<HTMLButtonElement>(
			container.querySelector("[data-slot='workbench-sidebar-settings']"),
		);
		const terminalBefore = requiredElement<HTMLElement>(
			container.querySelector("[data-test-terminal-surface='true']"),
		);
		settingsAction.focus();
		await act(async () => settingsAction.click());
		await vi.waitFor(() => expect(terminalSettingsController.getSnapshot().open).toBe(true));

		const workbench = requiredElement<HTMLElement>(
			container.querySelector("[data-slot='web-application-workbench']"),
		);
		const surface = requiredElement<HTMLElement>(
			container.querySelector("[data-slot='terminal-settings-surface']"),
		);
		const terminalWhileOpen = requiredElement<HTMLElement>(
			container.querySelector("[data-test-terminal-surface='true']"),
		);
		expect(terminalWhileOpen).toBe(terminalBefore);
		expect(workbench.inert).toBe(true);
		expect(workbench.getAttribute('aria-hidden')).toBe('true');
		expect(document.activeElement).toBe(
			container.querySelector("input[aria-label='Search settings']"),
		);

		await act(async () => {
			surface.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));
		});
		await vi.waitFor(() => expect(terminalSettingsController.getSnapshot().open).toBe(false));
		await vi.waitFor(() => expect(document.activeElement).toBe(settingsAction));
		expect(workbench.inert).toBe(false);
		expect(workbench.hasAttribute('aria-hidden')).toBe(false);
		expect(container.querySelector("[data-test-terminal-surface='true']")).toBe(terminalBefore);

		await act(async () => root.unmount());
		container.remove();
		keyboardShortcutSettingsController.dispose();
		terminalSettingsController.dispose();
		actionRuntime.dispose();
	});

	it('projects live Workbench and terminal bindings into the terminal context menu', async () => {
		const container = document.createElement('div');
		document.body.append(container);
		const actionRuntime = new ActionRuntime();
		const projectWorkspaceRuntime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1)]),
		);
		const terminalSettingsController = new TerminalSettingsController({
			storage: { getItem: () => null, setItem: () => undefined },
			themeTarget: document.documentElement,
			userPreferences: memoryUserPreferences(),
		});
		const keyboardShortcutSettingsController = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage: { getItem: () => null, setItem: () => undefined },
		});
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(
						TooltipProvider,
						undefined,
						createElement(WebApplication, {
							actionRuntime,
							applicationLocaleController: createApplicationLocaleController(),
							authority: inertAuthority,
							keyboardShortcutSettingsController,
							projectWorkspaceRuntime,
							shortcutPlatform: 'apple',
							terminalSettingsController,
							terminalSurface: ShortcutProjectionTerminalSurface,
						}),
					),
				),
			);
		});
		const terminalBefore = requiredElement<HTMLElement>(
			container.querySelector("[data-test-terminal-surface='true']"),
		);

		await act(async () => {
			keyboardShortcutSettingsController.setShortcut('workbench.panel.active.split-right', {
				code: 'KeyD',
				modifiers: ['mod', 'alt'],
			});
			keyboardShortcutSettingsController.setShortcut('terminal.clipboard.paste', {
				code: 'KeyV',
				modifiers: ['mod', 'shift'],
			});
		});
		expect(container.querySelector("[data-test-terminal-surface='true']")).toBe(terminalBefore);

		const viewport = requiredElement<HTMLElement>(
			container.querySelector("[data-slot='workbench-terminal-viewport']"),
		);
		await act(async () => {
			viewport.dispatchEvent(
				new MouseEvent('contextmenu', {
					bubbles: true,
					button: 2,
					clientX: 40,
					clientY: 40,
				}),
			);
		});
		await vi.waitFor(() => expect(document.querySelector('[role="menu"]')).not.toBeNull());

		expect(menuShortcut('Split right')).toBe('⌥⌘D');
		expect(menuShortcut('Paste')).toBe('⇧⌘V');
		expect(menuShortcut('Close')).toBe('⌘W');
		expect(menuShortcut('New child terminal')).toBeNull();

		await act(async () => root.unmount());
		container.remove();
		keyboardShortcutSettingsController.dispose();
		terminalSettingsController.dispose();
		actionRuntime.dispose();
	});
});

function TestTerminalSurface(properties: WorkbenchTerminalSurfaceProps): ReactNode {
	const { presentation } = properties;
	useImmediateTerminalPresentationReadiness(properties);
	return createElement(
		'div',
		{
			'data-presentation-id': presentation.presentationId,
			'data-test-terminal-surface': 'true',
		},
		'Terminal surface',
	);
}

function FocusTerminalSurface({
	autoFocus,
	focusRequest,
	onPresentationReady,
	presentation,
	presentationMode,
}: WorkbenchTerminalSurfaceProps): ReactNode {
	const inputReference = useRef<HTMLTextAreaElement>(null);
	useImmediateTerminalPresentationReadiness({ onPresentationReady, presentationMode });
	useEffect(() => {
		if (autoFocus) inputReference.current?.focus();
	}, [autoFocus, focusRequest, presentation.presentationId]);
	return createElement('textarea', {
		'aria-label': presentation.title,
		'data-presentation-id': presentation.presentationId,
		'data-test-terminal-input': 'true',
		ref: inputReference,
	});
}

function ShortcutProjectionTerminalSurface({
	actionRuntime,
	actionScope,
	bindActionContextProvider,
	onPresentationReady,
	presentation,
	presentationMode,
}: WorkbenchTerminalSurfaceProps): ReactNode {
	useImmediateTerminalPresentationReadiness({ onPresentationReady, presentationMode });
	useEffect(() => {
		const actionRegistration = actionRuntime.register(
			actionScope,
			defineAction({
				category: {
					id: 'terminal',
					title: { defaultMessage: 'Terminal', id: 'test.terminal.category' },
				},
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'test-paste',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: { execute: () => ({ kind: 'completed' }) },
				id: 'terminal.clipboard.paste',
				input: noActionInput,
				title: { defaultMessage: 'Paste', id: 'test.terminal.paste' },
			}),
		);
		const contextRegistration = bindActionContextProvider({
			capture: () => ({ focus: { terminal: true }, kind: 'product' }),
			executePrepared: (invocation) => actionRuntime.executePrepared(invocation),
		});
		return () => {
			contextRegistration.dispose();
			actionRegistration.dispose();
		};
	}, [actionRuntime, actionScope, bindActionContextProvider]);

	return createElement(
		'div',
		{
			'data-presentation-id': presentation.presentationId,
			'data-test-terminal-surface': 'true',
		},
		'Terminal surface',
	);
}

function useImmediateTerminalPresentationReadiness({
	onPresentationReady,
	presentationMode,
}: Pick<WorkbenchTerminalSurfaceProps, 'onPresentationReady' | 'presentationMode'>): void {
	useLayoutEffect(() => {
		if (presentationMode !== 'incoming') return;
		onPresentationReady('display-ready');
		onPresentationReady('activation-ready');
	}, [onPresentationReady, presentationMode]);
}

const inertAuthority = inertWorkbenchAuthority();

function createApplicationLocaleController(): ApplicationLocaleController {
	return new ApplicationLocaleController({
		documentElement: document.documentElement,
		preferredLocales: ['en-US'],
		userPreferences: memoryUserPreferences(),
	});
}

function menuShortcut(label: string): string | null {
	const item = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find((candidate) =>
		candidate.textContent?.startsWith(label),
	);
	if (item === undefined) throw new Error(`Expected menu item: ${label}`);
	return (
		item.querySelector<HTMLElement>('[data-slot="context-menu-shortcut"]')?.textContent ?? null
	);
}

function requiredElement<ElementType extends Element>(element: ElementType | null): ElementType {
	if (element === null) throw new Error('Expected element was not rendered.');
	return element;
}

function appleCommand(code: string): KeyboardEvent {
	return new KeyboardEvent('keydown', {
		bubbles: true,
		cancelable: true,
		code,
		metaKey: true,
	});
}
