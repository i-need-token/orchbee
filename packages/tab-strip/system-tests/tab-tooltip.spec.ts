import { expect, test } from '@playwright/test';

test('a Tab names a hidden or clipped title in the shared tooltip after the delay', async ({
	page,
}) => {
	await page.goto('/?tabTooltips');
	const tooltip = page.locator('[data-slot="tooltip-content"]');
	const pinned = page.locator('[data-tab-id="mail"]');
	await expect(pinned).toBeVisible();
	// The Tab and its title carry no native tooltip; inner controls keep their own names.
	const nativeTitles = await page.locator('[data-tab-id]').evaluateAll((nodes) =>
		nodes
			.flatMap((node) => [node, ...node.querySelectorAll('[data-tab-title]')])
			.map((node) => node.getAttribute('title') ?? '')
			.filter((title) => title !== ''),
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
});
