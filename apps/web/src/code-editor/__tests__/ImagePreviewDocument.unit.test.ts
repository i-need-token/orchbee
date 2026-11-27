// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';

import type { FileContent, FileResult } from '../../file-session';
import { ImagePreviewDocument } from '../ImagePreviewDocument';

const PNG: FileResult<FileContent> = {
	kind: 'ok',
	value: {
		bytes: new Uint8Array([0x89, 0x50, 0x4e, 0x47]),
		changed: false,
		kind: 'bytes',
		version: { mtimeMs: 1, size: 4 },
	},
};
const DENIED: FileResult<FileContent> = { error: { code: 'permission-denied' }, kind: 'error' };

/** A document whose reads answer from a queue the test fills before each read settles. */
function harness(answers: FileResult<FileContent>[]) {
	const signals: AbortSignal[] = [];
	const document = new ImagePreviewDocument({
		path: '/project/logo.PNG',
		read: (signal) => {
			signals.push(signal);
			const answer = answers.shift();
			return answer === undefined ? new Promise(() => undefined) : Promise.resolve(answer);
		},
	});
	const settled = (): Promise<void> =>
		new Promise((resolve) => {
			const stop = document.subscribe(() => {
				if (document.getState().kind === 'loading') return;
				stop();
				resolve();
			});
		});
	return { document, settled, signals };
}

describe('ImagePreviewDocument', () => {
	it('shows the bytes as a data URL and keeps the shown image when a reload fails', async () => {
		const { document, settled } = harness([PNG, DENIED]);
		await settled();
		const shown = document.getState();
		expect(shown).toEqual({ kind: 'image', size: 4, source: 'data:image/png;base64,iVBORw==' });

		document.reload();
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(document.getState()).toBe(shown);
	});

	it('reports a failed first read and reads again on Try Again', async () => {
		const { document, settled } = harness([DENIED, PNG]);
		await settled();
		expect(document.getState()).toEqual({ error: { code: 'permission-denied' }, kind: 'error' });

		document.retry();
		expect(document.getState()).toEqual({ kind: 'loading' });
		await settled();
		expect(document.getState().kind).toBe('image');
	});

	it('abandons its read when disposed', () => {
		const { document, signals } = harness([]);
		document.dispose();
		expect(signals[0]?.aborted).toBe(true);
	});
});
