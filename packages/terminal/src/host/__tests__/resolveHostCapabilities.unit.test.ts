import { describe, expect, it } from 'vitest';
import { resolveHostCapabilities } from '../resolveHostCapabilities';

describe('resolveHostCapabilities', () => {
	it('derives methods and honors explicit capability denial', () => {
		expect(
			resolveHostCapabilities({
				clipboard: { read: async () => null, write: async () => {} },
				openTarget: async () => {},
				capabilities: { clipboardRead: false },
			}),
		).toEqual({
			clipboardRead: false,
			clipboardWrite: true,
			contextMenu: false,
			notifications: false,
			openTarget: true,
			pasteConfirmation: false,
			pointer: false,
			fileDrop: false,
		});
	});
});
