// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import type { FileContent, FileResult } from '../../../../file-session';
import { CodeEditorViewStates } from '../../../CodeEditorViewStates';
import { ImagePreviewDocument } from '../../../ImagePreviewDocument';
import type { ImageViewerViewState } from '../../../ImageViewer';
import { ImagePreview } from '../ImagePreview';

const decode = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, 'decode');

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
	// The bytes below are no image, so the decoder refuses them as a browser's would.
	Object.defineProperty(HTMLImageElement.prototype, 'decode', {
		configurable: true,
		value: () => Promise.reject(new Error('Not an image.')),
	});
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
	if (decode === undefined) Reflect.deleteProperty(HTMLImageElement.prototype, 'decode');
	else Object.defineProperty(HTMLImageElement.prototype, 'decode', decode);
});

const NOT_AN_IMAGE: FileResult<FileContent> = {
	kind: 'ok',
	value: {
		bytes: new Uint8Array([0x6e, 0x6f]),
		changed: false,
		kind: 'bytes',
		version: { mtimeMs: 1, size: 2 },
	},
};
const DENIED: FileResult<FileContent> = { error: { code: 'permission-denied' }, kind: 'error' };

async function until(condition: () => boolean): Promise<void> {
	for (let attempt = 0; attempt < 20 && !condition(); attempt += 1) {
		await act(async () => {
			await new Promise((resolve) => setTimeout(resolve, 0));
		});
	}
	expect(condition()).toBe(true);
}

function button(container: HTMLElement, label: string): HTMLButtonElement {
	const found = [...container.querySelectorAll('button')].find(
		(candidate) => candidate.textContent === label,
	);
	if (found === undefined) throw new Error(`No ${label} button.`);
	return found;
}

describe('ImagePreview', () => {
	it('offers Try Again after a failed read and the text editor for content that is no image', async () => {
		const answers = [DENIED, NOT_AN_IMAGE];
		const document = new ImagePreviewDocument({
			path: '/project/logo.png',
			read: () => Promise.resolve(answers.shift() ?? DENIED),
		});
		const openAsText = vi.fn<() => void>();
		const container = window.document.createElement('div');
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(ImagePreview, {
						document,
						name: 'logo.png',
						onOpenAsText: openAsText,
						platform: 'standard',
						viewStateKey: 'logo',
						viewStates: new CodeEditorViewStates<ImageViewerViewState>(),
					}),
				),
			);
		});

		await until(() => container.textContent.includes('unexpected error'));
		await act(async () => button(container, 'Try Again').click());
		await until(() => container.textContent.includes('An error occurred while loading the image.'));
		await act(async () => button(container, 'Open in Text Editor').click());
		expect(openAsText).toHaveBeenCalledTimes(1);

		await act(async () => root.unmount());
		document.dispose();
	});
});
