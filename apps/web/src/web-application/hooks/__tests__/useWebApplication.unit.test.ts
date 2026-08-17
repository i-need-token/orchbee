// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import {
	createWorkbenchPreviewTerminalPresentationComposition,
	terminalPresentationId,
	WorkbenchProjectWorkspaceRuntime,
	type WorkbenchTerminalSurfaceProps,
} from '@orchbee/workbench';
import { act, createElement, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { TerminalSettingsController } from '../../../terminal-settings';
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
});

function TestTerminalSurface({ presentation }: WorkbenchTerminalSurfaceProps): ReactNode {
	return createElement(
		'div',
		{
			'data-presentation-id': presentation.presentationId,
			'data-test-terminal-surface': 'true',
		},
		'Terminal surface',
	);
}

function requiredElement<ElementType extends Element>(element: ElementType | null): ElementType {
	if (element === null) throw new Error('Expected element was not rendered.');
	return element;
}
