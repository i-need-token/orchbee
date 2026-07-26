import { expect, test, type Locator, type Page } from '@playwright/test';

const root = (page: Page) => page.locator('[data-ct-root]');
const tab = (page: Page, id: string) => root(page).locator(`[data-tab-id="${id}"]`);
const newTabButton = (page: Page) => page.getByRole('button', { name: 'New tab', exact: true });

async function centerOf(locator: Locator): Promise<Readonly<{ x: number; y: number }>> {
	const box = await locator.boundingBox();
	if (!box) throw new Error('The drag target has no rendered bounds.');
	return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

async function tabPrecedes(page: Page, leadingId: string, trailingId: string): Promise<boolean> {
	return root(page)
		.locator('[data-horizontal-track] > [data-horizontal-item] [data-tab-id]')
		.evaluateAll(
			(elements, ids) => {
				const order = elements.map((element) => (element as HTMLElement).dataset.tabId);
				return order.indexOf(ids.leadingId) < order.indexOf(ids.trailingId);
			},
			{ leadingId, trailingId },
		);
}

test.use({ reducedMotion: 'no-preference' });

test('keeps the new-tab button fixed while the last tab moves left', async ({ page }) => {
	await page.goto('/');
	const draggedTab = tab(page, 'new-tab');
	const targetTab = tab(page, 'issue');
	const button = newTabButton(page);
	const initialButtonBox = await button.boundingBox();
	if (!initialButtonBox) throw new Error('The new-tab button has no rendered bounds.');

	const start = await centerOf(draggedTab);
	await page.mouse.move(start.x, start.y);
	await page.mouse.down();
	await page.mouse.move(start.x - 18, start.y, { steps: 3 });
	await expect(root(page)).toHaveAttribute('data-dragging', 'true');

	const destination = await centerOf(targetTab);
	await page.mouse.move(destination.x, destination.y, { steps: 12 });
	await expect.poll(() => tabPrecedes(page, 'new-tab', 'issue')).toBe(true);
	const reorderedButtonBox = await button.boundingBox();
	expect(reorderedButtonBox?.x).toBeCloseTo(initialButtonBox.x, 1);

	await page.mouse.up();
	await expect(page.locator('[data-horizontal-drag-context][data-settling]')).toHaveCount(1);
	const settlingButtonBox = await button.boundingBox();
	expect(settlingButtonBox?.x).toBeCloseTo(initialButtonBox.x, 1);
});
