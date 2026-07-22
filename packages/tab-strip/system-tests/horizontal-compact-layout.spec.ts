import { expect, test, type Page } from '@playwright/test';

const tabs = (page: Page) => page.locator('.ct-tab');
const activeTab = (page: Page) => page.locator('.ct-tab[data-active]');

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

test('compact active controls and separators cross their boundaries deterministically', async ({
	page,
}) => {
	await addTabs(page, 10);

	const active = activeTab(page);
	await expect(active).not.toHaveAttribute('data-hovered', 'true');
	await expect(active.locator('.ct-tab__close')).toHaveCount(1);
	await expect(active.locator('.ct-tab-icon')).toHaveCount(1);

	const iconBox = await active.locator('.ct-tab-icon').boundingBox();
	const closeBox = await active.locator('.ct-tab__close').boundingBox();
	expect(iconBox).not.toBeNull();
	expect(closeBox).not.toBeNull();
	if (!iconBox || !closeBox) throw new Error('Expected active tab controls to have bounds.');
	expect(closeBox.x).toBeLessThanOrEqual(iconBox.x + iconBox.width);

	await addTabs(page, 1);
	expect(
		await page
			.locator('.ct-tab__separator')
			.evaluateAll((elements) =>
				elements.some((element) => (element as HTMLElement).style.opacity === '1'),
			),
	).toBe(true);

	await addTabs(page, 1);
	expect(
		await page
			.locator('.ct-tab__separator')
			.evaluateAll((elements) =>
				elements.every((element) => (element as HTMLElement).style.opacity === '0'),
			),
	).toBe(true);
	await expect(activeTab(page).locator('.ct-tab__close')).toHaveCount(1);
	await expect(activeTab(page).locator('.ct-tab-icon')).toHaveCount(0);
});
