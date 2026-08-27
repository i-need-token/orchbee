// @vitest-environment jsdom

import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { TerminalCommandOutputViewer } from '../TerminalCommandOutputViewer';
import { TerminalCommandOutputViewerController } from '../model/terminal-command-output-viewer-controller';

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
});

describe('TerminalCommandOutputViewer', () => {
	it('searches immutable output, cycles matches, copies, traps focus, and restores focus on close', async () => {
		const controller = new TerminalCommandOutputViewerController();
		const onCopy = vi.fn<() => void>();
		const restoreFocus = vi.fn<() => void>();
		controller.open({
			endRow: 11n,
			generation: 2n,
			kind: 'output',
			revision: 7n,
			startRow: 8n,
			text: 'Alpha beta\nsecond ALPHA',
			truncatedStart: true,
		});
		const mounted = await renderViewer(controller, { onCopy, restoreFocus });

		const input = requiredElement<HTMLInputElement>(mounted.container.querySelector('input'));
		expect(document.activeElement).toBe(input);
		expect(mounted.container.querySelector('pre')?.textContent).toBe('Alpha beta\nsecond ALPHA');
		expect(mounted.container.textContent).toContain(
			'Earlier lines were evicted from retained history.',
		);

		await act(async () => {
			setInputValue(input, 'alpha');
			input.dispatchEvent(new Event('input', { bubbles: true }));
			await Promise.resolve();
		});
		expect(mounted.container.querySelectorAll('mark')).toHaveLength(2);
		expect(mounted.container.querySelector('mark[data-active="true"]')?.textContent).toBe('Alpha');
		expect(mounted.container.textContent).toContain('1 of 2 matches');

		await act(async () => {
			input.dispatchEvent(
				new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Enter' }),
			);
			await Promise.resolve();
		});
		expect(mounted.container.querySelector('mark[data-active="true"]')?.textContent).toBe('ALPHA');

		const content = requiredElement<HTMLElement>(
			mounted.container.querySelector('[aria-label="Last command output"]'),
		);
		await act(async () => {
			input.dispatchEvent(
				new KeyboardEvent('keydown', {
					bubbles: true,
					cancelable: true,
					key: 'Tab',
					shiftKey: true,
				}),
			);
		});
		expect(document.activeElement).toBe(content);
		await act(async () => {
			content.dispatchEvent(
				new KeyboardEvent('keydown', {
					bubbles: true,
					cancelable: true,
					code: 'KeyF',
					key: 'f',
					metaKey: true,
				}),
			);
			await Promise.resolve();
		});
		expect(document.activeElement).toBe(input);

		const copy = requiredElement<HTMLButtonElement>(
			mounted.container.querySelector('button[aria-label="Copy command output"]'),
		);
		await act(async () => copy.click());
		expect(onCopy).toHaveBeenCalledOnce();

		await act(async () => {
			input.dispatchEvent(
				new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Escape' }),
			);
			await Promise.resolve();
		});
		expect(mounted.container.querySelector('[role="dialog"]')).toBeNull();
		expect(restoreFocus).toHaveBeenCalledOnce();
		await mounted.dispose();
	});

	it('localizes an unavailable document and focuses its only close action', async () => {
		const controller = new TerminalCommandOutputViewerController();
		controller.open({ kind: 'empty', reason: 'no-shell-integration' });
		const mounted = await renderViewer(controller, { locale: 'zh-CN' });

		const close = requiredElement<HTMLButtonElement>(
			mounted.container.querySelector('button[aria-label="关闭命令输出"]'),
		);
		expect(document.activeElement).toBe(close);
		expect(mounted.container.textContent).toContain('此终端没有可用的 Shell 命令标记。');
		expect(mounted.container.querySelector('input')).toBeNull();
		expect(mounted.container.querySelector('button[aria-label="复制命令输出"]')).toBeNull();
		await mounted.dispose();
	});
});

interface RenderViewerOptions {
	readonly locale?: OrchbeeLocale;
	readonly onCopy?: () => void;
	readonly restoreFocus?: () => void;
}

async function renderViewer(
	controller: TerminalCommandOutputViewerController,
	{
		locale = 'en-US',
		onCopy = () => undefined,
		restoreFocus = () => undefined,
	}: RenderViewerOptions = {},
): Promise<{ readonly container: HTMLElement; dispose(): Promise<void> }> {
	const container = document.createElement('div');
	document.body.appendChild(container);
	const root = createRoot(container);
	await act(async () => {
		root.render(
			createElement(
				OrchbeeI18nProvider,
				{ locale },
				createElement(
					TooltipProvider,
					undefined,
					createElement(TerminalCommandOutputViewer, {
						controller,
						onCopy,
						platform: 'apple',
						restoreFocus,
					}),
				),
			),
		);
		await Promise.resolve();
	});
	return {
		container,
		dispose: () => disposeViewer(root, container),
	};
}

async function disposeViewer(root: Root, container: HTMLElement): Promise<void> {
	await act(async () => root.unmount());
	container.remove();
}

function requiredElement<ElementType extends Element>(element: ElementType | null): ElementType {
	if (element === null) throw new Error('Expected command-output viewer element.');
	return element;
}

function setInputValue(input: HTMLInputElement, value: string): void {
	const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
	setter?.call(input, value);
}
