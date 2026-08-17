// @vitest-environment jsdom

import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import type { ProjectedPasteReview } from '@orchbee/terminal';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { ProjectedTerminalPasteReviewDialog } from '../ProjectedTerminalPasteReviewDialog';

const review: ProjectedPasteReview = {
	bracketed: true,
	embeddedBracketTerminator: true,
	multiline: false,
	originalText: 'prefix\u001b[201~suffix',
	replacedControlCharacters: 1,
	source: 'clipboard',
	text: 'prefix [201~suffix',
};

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

describe('ProjectedTerminalPasteReviewDialog', () => {
	it.each([
		{ locale: 'en-US', title: 'Review terminal paste' },
		{ locale: 'zh-CN', title: '检查终端粘贴内容' },
	] satisfies readonly { readonly locale: OrchbeeLocale; readonly title: string }[])(
		'renders control-visible original text and actions in $locale',
		async ({ locale, title }) => {
			const approve = vi.fn<() => void>();
			const cancel = vi.fn<() => void>();
			const container = document.createElement('div');
			document.body.appendChild(container);
			const root = createRoot(container);

			await act(async () => {
				root.render(
					createElement(
						OrchbeeI18nProvider,
						{ locale },
						createElement(ProjectedTerminalPasteReviewDialog, {
							onApprove: approve,
							onCancel: cancel,
							review,
						}),
					),
				);
				await Promise.resolve();
			});

			const dialog = document.querySelector('[data-slot="projected-terminal-paste-review"]');
			if (!(dialog instanceof HTMLElement)) throw new Error('Paste review dialog did not open.');
			expect(dialog.textContent).toContain(title);
			expect(dialog.querySelector('pre')?.textContent).toBe('prefix\\u001B[201~suffix');
			const approveButton = dialog.querySelector('[data-slot="alert-dialog-action"]');
			if (!(approveButton instanceof HTMLButtonElement)) {
				throw new Error('Paste review approval is missing.');
			}
			await act(async () => {
				approveButton.click();
			});
			expect(approve).toHaveBeenCalledOnce();
			const cancelButton = dialog.querySelector('[data-slot="alert-dialog-cancel"]');
			if (!(cancelButton instanceof HTMLButtonElement)) {
				throw new Error('Paste review cancellation is missing.');
			}
			await act(async () => {
				cancelButton.click();
			});
			expect(cancel).toHaveBeenCalledOnce();

			await act(async () => root.unmount());
			container.remove();
		},
	);

	it.each([
		{
			confirm: 'Paste paths',
			locale: 'en-US',
			remote: 'These are paths on this device.',
			title: 'Review dropped paths',
		},
		{
			confirm: '粘贴路径',
			locale: 'zh-CN',
			remote: '这些路径属于当前设备',
			title: '检查拖入路径',
		},
	] satisfies readonly {
		readonly confirm: string;
		readonly locale: OrchbeeLocale;
		readonly remote: string;
		readonly title: string;
	}[])(
		'renders mandatory local-path and remote-shell context in $locale',
		async ({ confirm, locale, remote, title }) => {
			const container = document.createElement('div');
			document.body.appendChild(container);
			const root = createRoot(container);
			await act(async () => {
				root.render(
					createElement(
						OrchbeeI18nProvider,
						{ locale },
						createElement(ProjectedTerminalPasteReviewDialog, {
							onApprove: () => undefined,
							onCancel: () => undefined,
							review: {
								bracketed: false,
								embeddedBracketTerminator: false,
								multiline: true,
								originalText: "'/tmp/line\nbreak.txt'",
								replacedControlCharacters: 0,
								source: 'file-drop',
								text: "'/tmp/line\nbreak.txt'",
							},
						}),
					),
				);
			});

			const dialog = document.querySelector('[data-slot="projected-terminal-paste-review"]');
			expect(dialog?.textContent).toContain(title);
			expect(dialog?.textContent).toContain(remote);
			expect(dialog?.textContent).toContain(confirm);
			expect(dialog?.textContent).not.toContain('may run multiple commands');
			await act(async () => root.unmount());
			container.remove();
		},
	);
});
