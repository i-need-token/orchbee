// @vitest-environment jsdom

import { ActionRuntime, defineAction, noActionInput } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import {
	createWorkbenchPreviewTerminalPresentationComposition,
	WorkbenchProjectWorkspaceRuntime,
	type WorkbenchTerminalSurfaceProps,
} from '@orchbee/workbench';
import { act, createElement, useEffect, useLayoutEffect, useRef, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import { TerminalSettingsController } from '../../../terminal-settings';
import { ApplicationLocaleController } from '../../../application-locale';
import { KeyboardShortcutSettingsController } from '../../../keyboard-shortcuts';
import { WebApplication } from '../../WebApplication';

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
		let sequence = 0;
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/workspace',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
				terminalPresentationId('shortcut-scope:initial'),
				() => terminalPresentationId(`shortcut-scope:${(sequence += 1)}`),
			),
		});
		const terminalSettingsController = new TerminalSettingsController({
			storage: { getItem: () => null, setItem: () => undefined },
			themeTarget: document.documentElement,
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

		const initialInput = requiredElement<HTMLTextAreaElement>(
			container.querySelector('[data-test-terminal-input="true"]'),
		);
		expect(document.activeElement).toBe(initialInput);
		initialInput.blur();
		expect(document.activeElement).toBe(document.body);
		const createWithBodyFocus = appleCommand('KeyT');
		await act(async () => document.body.dispatchEvent(createWithBodyFocus));
		expect(createWithBodyFocus.defaultPrevented).toBe(true);
		await vi.waitFor(() =>
			expect(
				projectWorkspaceRuntime.getActiveTerminalContext().tabModel.getSnapshot().tabs,
			).toHaveLength(2),
		);
		const createdInput = requiredElement<HTMLTextAreaElement>(
			container.querySelector('[data-test-terminal-input="true"]'),
		);
		expect(document.activeElement).toBe(createdInput);
		createdInput.blur();

		const split = appleCommand('KeyD');
		await act(async () => document.body.dispatchEvent(split));
		expect(split.defaultPrevented).toBe(true);
		await vi.waitFor(() =>
			expect(container.querySelectorAll('[data-slot="workbench-panel"]')).toHaveLength(2),
		);
		const splitFocus = document.activeElement;
		expect(splitFocus).toBeInstanceOf(HTMLTextAreaElement);
		if (!(splitFocus instanceof HTMLTextAreaElement)) {
			throw new Error('Expected the split terminal to receive focus.');
		}
		splitFocus.blur();

		const closePanel = appleCommand('KeyW');
		await act(async () => document.body.dispatchEvent(closePanel));
		expect(closePanel.defaultPrevented).toBe(true);
		await confirmClose();
		await vi.waitFor(() =>
			expect(container.querySelectorAll('[data-slot="workbench-panel"]')).toHaveLength(1),
		);
		await vi.waitFor(() =>
			expect(document.activeElement).toBe(
				container.querySelector('[data-test-terminal-input="true"]'),
			),
		);

		const activeTabInput = requiredElement<HTMLTextAreaElement>(
			container.querySelector('[data-test-terminal-input="true"]'),
		);
		activeTabInput.blur();
		const closeActiveTab = appleCommand('KeyW');
		await act(async () => document.body.dispatchEvent(closeActiveTab));
		expect(closeActiveTab.defaultPrevented).toBe(true);
		await confirmClose();
		await vi.waitFor(() =>
			expect(
				projectWorkspaceRuntime.getActiveTerminalContext().tabModel.getSnapshot().tabs,
			).toHaveLength(1),
		);
		const finalTabInput = requiredElement<HTMLTextAreaElement>(
			container.querySelector('[data-test-terminal-input="true"]'),
		);
		expect(document.activeElement).toBe(finalTabInput);
		finalTabInput.blur();
		const closeFinalTab = appleCommand('KeyW');
		await act(async () => document.body.dispatchEvent(closeFinalTab));
		expect(closeFinalTab.defaultPrevented).toBe(true);
		await confirmClose();
		await vi.waitFor(() =>
			expect(
				projectWorkspaceRuntime.getActiveTerminalContext().tabModel.getSnapshot().tabs,
			).toHaveLength(0),
		);
		expect(
			projectWorkspaceRuntime.getActiveTerminalContext().tabModel.getSnapshot().tabs,
		).toHaveLength(0);
		expect(document.activeElement).toBe(document.body);

		const createFromEmpty = appleCommand('KeyT');
		await act(async () => document.body.dispatchEvent(createFromEmpty));
		expect(createFromEmpty.defaultPrevented).toBe(true);
		await vi.waitFor(() =>
			expect(
				projectWorkspaceRuntime.getActiveTerminalContext().tabModel.getSnapshot().tabs,
			).toHaveLength(1),
		);
		const recreatedInput = requiredElement<HTMLTextAreaElement>(
			container.querySelector('[data-test-terminal-input="true"]'),
		);
		expect(document.activeElement).toBe(recreatedInput);
		const increaseFontSize = appleCommand('Equal');
		await act(async () => recreatedInput.dispatchEvent(increaseFontSize));
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
		await vi.waitFor(() => expect(document.activeElement).toBe(recreatedInput));
		const resetFontSize = appleCommand('Digit0');
		await act(async () => recreatedInput.dispatchEvent(resetFontSize));
		expect(resetFontSize.defaultPrevented).toBe(true);
		expect(terminalSettingsController.getSnapshot().preferences.fontSize).toBe(initialFontSize);
		recreatedInput.blur();
		const createAfterSettings = appleCommand('KeyT');
		await act(async () => document.body.dispatchEvent(createAfterSettings));
		expect(createAfterSettings.defaultPrevented).toBe(true);
		await vi.waitFor(() =>
			expect(
				projectWorkspaceRuntime.getActiveTerminalContext().tabModel.getSnapshot().tabs,
			).toHaveLength(2),
		);
		expect(document.activeElement).toBe(
			container.querySelector('[data-test-terminal-input="true"]'),
		);

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
		let sequence = 0;
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/workspace',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
				terminalPresentationId('settings-integration:initial'),
				() => terminalPresentationId(`settings-integration:${(sequence += 1)}`),
			),
		});
		const terminalSettingsController = new TerminalSettingsController({
			storage: { getItem: () => null, setItem: () => undefined },
			themeTarget: document.documentElement,
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
		let sequence = 0;
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/workspace',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
				terminalPresentationId('shortcut-projection:initial'),
				() => terminalPresentationId(`shortcut-projection:${(sequence += 1)}`),
			),
		});
		const terminalSettingsController = new TerminalSettingsController({
			storage: { getItem: () => null, setItem: () => undefined },
			themeTarget: document.documentElement,
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

function createApplicationLocaleController(): ApplicationLocaleController {
	return new ApplicationLocaleController({
		documentElement: document.documentElement,
		preferredLocales: ['en-US'],
		storage: { getItem: () => null, setItem: () => undefined },
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

async function confirmClose(): Promise<void> {
	const close = await vi.waitFor(() =>
		requiredElement<HTMLButtonElement>(
			[...document.querySelectorAll<HTMLButtonElement>('[role="alertdialog"] button')].find(
				(button) => button.textContent?.trim() === 'Close',
			) ?? null,
		),
	);
	await act(async () => close.click());
}
