import { expect, test } from '@playwright/test';

test('a Tab, group heading, unread dot, and alert indicator use the shared tooltip after the delay', async ({
	page,
}) => {
	await page.goto('/?tabTooltips');
	const tooltip = page.locator('[data-slot="tooltip-content"]');
	const pinned = page.locator('[data-tab-id="mail"]');
	await expect(pinned).toBeVisible();
	// Tabs, groups, unread dots, and alert indicators carry no native title tooltips.
	const nativeTitles = await page
		.locator(
			'[data-tab-id], [data-tab-title], [data-tab-strip-group-header], [data-tab-unread], [data-tab-alert-indicator]',
		)
		.evaluateAll((nodes) =>
			nodes.map((node) => node.getAttribute('title') ?? '').filter((title) => title !== ''),
		);
	expect(nativeTitles).toEqual([]);

	await pinned.hover();
	await page.waitForTimeout(150);
	await expect(tooltip).toHaveCount(0);
	await expect(tooltip).toHaveText('Inbox (3) – Example Mail');
	await expect(page.locator('[data-tab-strip-hover-card]')).toHaveCount(0);

	await page.mouse.move(700, 600);
	await expect(tooltip).toHaveCount(0);
	await page.locator('[data-tab-id="source"]').hover();
	await page.waitForTimeout(600);
	await expect(tooltip).toHaveCount(0);

	await page.mouse.move(700, 600);
	await expect(tooltip).toHaveCount(0);
	await page.locator('[data-tab-strip-group-header][data-group-id="research"]').hover();
	await page.waitForTimeout(150);
	await expect(tooltip).toHaveCount(0);
	await expect(tooltip).toHaveText('Research - orchbee / orchbee and 1 other tab');

	await page.mouse.move(700, 600);
	await expect(tooltip).toHaveCount(0);
	await page.waitForTimeout(450);
	await page.locator('[data-tab-id="calendar"] [data-tab-unread]').hover();
	await page.waitForTimeout(150);
	await expect(tooltip).toHaveCount(0);
	await expect(tooltip).toHaveText('Unread output');

	await page.mouse.move(700, 600);
	await expect(tooltip).toHaveCount(0);
	await page.waitForTimeout(450);
	await page.locator('[data-tab-id="preview"] [data-tab-alert-indicator]').hover();
	await page.waitForTimeout(150);
	await expect(tooltip).toHaveCount(0);
	await expect(tooltip).toHaveText('This tab is playing audio');
});
