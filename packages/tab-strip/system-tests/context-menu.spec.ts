import { expect, test, type Locator, type Page } from '@playwright/test';

const root = (page: Page) => page.locator('[data-ct-root]');
const tab = (page: Page, id: string) => root(page).locator(`[data-tab-id="${id}"]`);
const group = (page: Page, id: string) =>
	root(page).locator(`[data-tab-strip-group-header][data-group-id="${id}"]`);
const tabOrder = (page: Page) =>
	root(page)
		.locator('[data-tab-id]')
		.evaluateAll((tabs) => tabs.map((tabElement) => tabElement.getAttribute('data-tab-id')));

async function openGroupSubmenu(page: Page, target: Locator): Promise<void> {
	await target.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Add to group', exact: true }).hover();
	await expect(page.getByRole('menuitem', { name: 'New group…', exact: true })).toBeVisible();
}

test.beforeEach(async ({ page }) => {
	await page.goto('/');
	await expect(root(page)).toHaveAttribute('data-orientation', 'horizontal');
});

test('group submenu commands commit with primary-pointer clicks', async ({ page }) => {
	const originalOrder = await tabOrder(page);
	await tab(page, 'issue').click();
	await openGroupSubmenu(page, tab(page, 'issue'));
	await page.getByRole('menuitem', { name: 'New group…', exact: true }).click();

	await expect(group(page, 'group-1')).toHaveCount(1);
	await expect(tab(page, 'issue')).toHaveAccessibleName(/unnamed group/i);
	await expect(root(page).locator('[data-group-underline-id="group-1"]')).toHaveCSS('top', '1px');
	await expect(tab(page, 'issue')).toHaveAttribute('data-grouped', 'true');
	await tab(page, 'issue').focus();
	await expect(tab(page, 'issue').locator('[data-tab-background-fill]')).toHaveCSS(
		'outline-style',
		'none',
	);
	await expect(
		tab(page, 'issue').locator('[style*="--ob-comp-tab-strip-runtime-stroke"]'),
	).toHaveCount(0);
	await expect.poll(() => tabOrder(page)).toEqual(originalOrder);

	await group(page, 'group-1').click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Group color', exact: true }).hover();
	await page.getByRole('menuitemradio', { name: 'Red', exact: true }).click();
	await expect(page.getByRole('menu')).toHaveCount(0);
	await group(page, 'group-1').click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Group color', exact: true }).hover();
	await expect(page.getByRole('menuitemradio', { name: 'Red', exact: true })).toHaveAttribute(
		'aria-checked',
		'true',
	);
	await page.keyboard.press('Escape');
	await page.keyboard.press('Escape');
	await expect(page.getByRole('menu')).toHaveCount(0);
	await group(page, 'group-1').click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Ungroup, keep 1 Tab', exact: true }).click();

	await expect(group(page, 'group-1')).toHaveCount(0);
	await expect(root(page).locator('[data-group-underline-id="group-1"]')).toHaveCount(0);
	await expect(
		tab(page, 'issue').locator('[style*="--ob-comp-tab-strip-runtime-stroke"]'),
	).toHaveCount(0);
	await expect(tab(page, 'issue')).not.toHaveAccessibleName(/group/i);
	await expect.poll(() => tabOrder(page)).toEqual(originalOrder);

	await openGroupSubmenu(page, tab(page, 'issue'));
	await page.getByRole('menuitem', { name: 'New group…', exact: true }).click();
	await expect(group(page, 'group-1')).toHaveCount(1);
	await tab(page, 'issue').click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Remove from group', exact: true }).click();
	await expect(group(page, 'group-1')).toHaveCount(0);
	await expect(root(page).locator('[data-group-underline-id="group-1"]')).toHaveCount(0);
	await expect(
		tab(page, 'issue').locator('[style*="--ob-comp-tab-strip-runtime-stroke"]'),
	).toHaveCount(0);
	await expect.poll(() => tabOrder(page)).toEqual(originalOrder);

	await openGroupSubmenu(page, tab(page, 'new-tab'));
	await page.getByRole('menuitem', { name: 'Research', exact: true }).click();

	await expect(tab(page, 'new-tab')).toHaveAccessibleName(/Research/);
});
