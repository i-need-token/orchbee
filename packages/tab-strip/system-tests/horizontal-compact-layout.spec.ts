import { expect, test, type Page } from '@playwright/test';

const tabs = (page: Page) => page.locator('[data-tab-id]');
const activeTab = (page: Page) => page.locator('[data-tab-id][data-active]');

async function addTabs(page: Page, count: number): Promise<void> {
	const button = page.getByRole('button', { name: 'New tab', exact: true });
	const initialCount = await tabs(page).count();
	for (let index = 0; index < count; index += 1) await button.click();
	await expect(tabs(page)).toHaveCount(initialCount + count);
}

test.beforeEach(async ({ page }) => {
	await page.setViewportSize({ width: 1280, height: 800 });
	await page.goto('/');
});

test('keeps the horizontal strip and its controls centered in a 36px row', async ({ page }) => {
	const strip = page.locator('[data-ct-root][data-orientation="horizontal"]');
	const track = page.locator('[data-horizontal-track]');
	const item = track.locator('[data-horizontal-item]').first();
	const groupChip = track.locator('[data-group-chip]').first();
	const newTabButton = page.getByRole('button', { name: 'New tab', exact: true });

	const [stripBox, trackBox, itemBox, groupChipBox, newTabButtonBox] = await Promise.all([
		strip.boundingBox(),
		track.boundingBox(),
		item.boundingBox(),
		groupChip.boundingBox(),
		newTabButton.boundingBox(),
	]);
	if (!stripBox || !trackBox || !itemBox || !groupChipBox || !newTabButtonBox) {
		throw new Error('Expected complete horizontal tab-strip geometry.');
	}

	expect(stripBox.height).toBe(36);
	expect(trackBox.height).toBe(36);
	expect(itemBox.height).toBe(36);
	expect(groupChipBox.y - stripBox.y).toBe(8);
	expect(newTabButtonBox.height).toBe(28);
	expect(newTabButtonBox.y - stripBox.y).toBe(4);
});

test('renders one shared separator between adjacent inactive tabs', async ({ page }) => {
	const trailing = page.locator('[data-tab-id="issue"] [data-tab-separator="trailing"]');
	const leading = page.locator('[data-tab-id="new-tab"] [data-tab-separator="leading"]');

	await expect(trailing).toHaveCSS('opacity', '1');
	await expect(leading).toHaveCSS('opacity', '0');
});

test('caps ordinary horizontal tabs at 200 pixels', async ({ page }) => {
	await page.setViewportSize({ width: 2400, height: 800 });

	await expect(page.locator('[data-tab-id="issue"]')).toHaveCSS('width', '200px');
	await expect(page.locator('[data-tab-id="new-tab"]')).toHaveCSS('width', '200px');
});

test('keeps an active group underline out of the adjacent collection', async ({ page }) => {
	const underlineBox = await page.locator('[data-group-underline-id="research"]').boundingBox();
	const adjacentGroupBox = await page.locator('[data-group-id="build"]').boundingBox();
	if (!underlineBox || !adjacentGroupBox) {
		throw new Error('Expected the group underline and adjacent group to have bounds.');
	}

	expect(underlineBox.x + underlineBox.width).toBeLessThanOrEqual(adjacentGroupBox.x);
});

test('compact active controls and separators cross their boundaries deterministically', async ({
	page,
}) => {
	await addTabs(page, 10);

	const active = activeTab(page);
	await expect(active).not.toHaveAttribute('data-hovered', 'true');
	await expect(active.getByRole('button', { name: 'Close' })).toHaveCount(1);
	await expect(active.locator('[data-tab-icon]')).toHaveCount(1);

	const iconBox = await active.locator('[data-tab-icon]').boundingBox();
	const closeBox = await active.getByRole('button', { name: 'Close' }).boundingBox();
	expect(iconBox).not.toBeNull();
	expect(closeBox).not.toBeNull();
	if (!iconBox || !closeBox) throw new Error('Expected active tab controls to have bounds.');
	expect(closeBox.x).toBeLessThanOrEqual(iconBox.x + iconBox.width + 2);

	await addTabs(page, 1);
	expect(
		await page
			.locator('[data-tab-separator]')
			.evaluateAll((elements) =>
				elements.some(
					(element) =>
						(element as HTMLElement).style.getPropertyValue(
							'--ob-comp-tab-strip-runtime-opacity',
						) === '1',
				),
			),
	).toBe(true);

	await addTabs(page, 1);
	expect(
		await page
			.locator('[data-tab-separator]')
			.evaluateAll((elements) =>
				elements.every(
					(element) =>
						(element as HTMLElement).style.getPropertyValue(
							'--ob-comp-tab-strip-runtime-opacity',
						) === '0',
				),
			),
	).toBe(true);
	await expect(activeTab(page).getByRole('button', { name: 'Close' })).toHaveCount(1);
	await expect(activeTab(page).locator('[data-tab-icon]')).toHaveCount(0);
});
