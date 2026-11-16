import { describe, expect, it, vi } from 'vitest';

import { createBrowserLocalFontCatalog } from '../createBrowserLocalFontCatalog';

describe('createBrowserLocalFontCatalog', () => {
	it('offers nothing where the browser has no Local Font Access', () => {
		expect(createBrowserLocalFontCatalog({})).toBeUndefined();
	});

	it('lists each installed family once, in name order', async () => {
		const queryLocalFonts = vi.fn<() => Promise<readonly { family: string; style: string }[]>>(() =>
			Promise.resolve([
				{ family: 'Consolas', style: 'Bold' },
				{ family: 'Arial', style: 'Regular' },
				{ family: 'Consolas', style: 'Regular' },
			]),
		);
		const catalog = createBrowserLocalFontCatalog({ queryLocalFonts });

		await expect(catalog?.listFamilies()).resolves.toEqual(['Arial', 'Consolas']);
		await catalog?.listFamilies();
		expect(queryLocalFonts).toHaveBeenCalledTimes(1);
	});

	it('answers null to a refusal and asks again the next time', async () => {
		const queryLocalFonts = vi
			.fn<() => Promise<readonly { family: string }[]>>()
			.mockRejectedValueOnce(new DOMException('Not allowed', 'NotAllowedError'))
			.mockResolvedValueOnce([{ family: 'Consolas' }]);
		const catalog = createBrowserLocalFontCatalog({ queryLocalFonts });

		await expect(catalog?.listFamilies()).resolves.toBeNull();
		await expect(catalog?.listFamilies()).resolves.toEqual(['Consolas']);
	});
});
