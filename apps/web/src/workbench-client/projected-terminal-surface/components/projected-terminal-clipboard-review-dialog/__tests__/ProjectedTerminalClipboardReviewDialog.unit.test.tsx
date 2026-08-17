// @vitest-environment jsdom

import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import {
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	type TerminalClipboardWriteRequest,
} from '@orchbee/terminal-protocol';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { ProjectedTerminalClipboardReviewDialog } from '../ProjectedTerminalClipboardReviewDialog';

const request: TerminalClipboardWriteRequest = {
	attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
	sequence: CanonicalUint64TextSchema.parse('7'),
	text: 'first line\n第二行',
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

describe('ProjectedTerminalClipboardReviewDialog', () => {
	it.each([
		{
			cancel: 'Keep current clipboard',
			confirm: 'Copy to clipboard',
			locale: 'en-US',
			title: 'Allow terminal clipboard write?',
		},
		{
			cancel: '保留当前剪贴板',
			confirm: '复制到剪贴板',
			locale: 'zh-CN',
			title: '允许终端写入剪贴板吗？',
		},
	] satisfies readonly {
		readonly cancel: string;
		readonly confirm: string;
		readonly locale: OrchbeeLocale;
		readonly title: string;
	}[])('renders the exact bounded request and explicit actions in $locale', async (copy) => {
		const approve = vi.fn<() => void>();
		const cancel = vi.fn<() => void>();
		const container = document.createElement('div');
		document.body.appendChild(container);
		const root = createRoot(container);

		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: copy.locale },
					createElement(ProjectedTerminalClipboardReviewDialog, {
						onApprove: approve,
						onCancel: cancel,
						request,
					}),
				),
			);
			await Promise.resolve();
		});

		const dialog = document.querySelector('[data-slot="projected-terminal-clipboard-review"]');
		if (!(dialog instanceof HTMLElement)) throw new Error('Clipboard review dialog did not open.');
		expect(dialog.textContent).toContain(copy.title);
		expect(dialog.textContent).toContain('20');
		expect(dialog.textContent).toContain('UTF-8');
		expect(dialog.querySelector('pre')?.textContent).toBe(request.text);
		const approveButton = dialog.querySelector('[data-slot="alert-dialog-action"]');
		if (!(approveButton instanceof HTMLButtonElement)) {
			throw new Error('Clipboard review approval is missing.');
		}
		expect(approveButton.textContent).toBe(copy.confirm);
		await act(async () => approveButton.click());
		expect(approve).toHaveBeenCalledOnce();

		const cancelButton = dialog.querySelector('[data-slot="alert-dialog-cancel"]');
		if (!(cancelButton instanceof HTMLButtonElement)) {
			throw new Error('Clipboard review cancellation is missing.');
		}
		expect(cancelButton.textContent).toBe(copy.cancel);
		await act(async () => cancelButton.click());
		expect(cancel).toHaveBeenCalledOnce();

		await act(async () => root.unmount());
		container.remove();
	});
});
