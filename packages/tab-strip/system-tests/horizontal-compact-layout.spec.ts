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
	expect(closeBox.x).toBeLessThanOrEqual(iconBox.x + iconBox.width + 1);

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
