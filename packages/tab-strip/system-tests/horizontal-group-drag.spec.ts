import { expect, test, type Locator, type Page } from '@playwright/test';

const root = (page: Page) => page.locator('[data-ct-root]');
const sourceHeader = (page: Page) =>
	root(page).locator(
		'[data-horizontal-track] > [data-horizontal-item] [data-tab-strip-group-header][data-group-id="research"]',
	);
const groupHeader = (page: Page, id: string) =>
	root(page).locator(
		`[data-horizontal-track] > [data-horizontal-item] [data-tab-strip-group-header][data-group-id="${id}"]`,
	);
const tab = (page: Page, id: string) => root(page).locator(`[data-tab-id="${id}"]`);

async function centerOf(locator: Locator): Promise<Readonly<{ x: number; y: number }>> {
	const box = await locator.boundingBox();
	if (!box) throw new Error('The drag target has no rendered bounds.');
	return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

async function beginDrag(page: Page, locator: Locator): Promise<void> {
	const start = await centerOf(locator);
	await page.mouse.move(start.x, start.y);
	await page.mouse.down();
	await page.mouse.move(start.x + 18, start.y, { steps: 3 });
	await expect(root(page)).toHaveAttribute('data-dragging', 'true');
}

async function movePointerTo(page: Page, locator: Locator): Promise<void> {
	const destination = await centerOf(locator);
	await page.mouse.move(destination.x, destination.y, { steps: 12 });
}

async function waitForSettledLayout(page: Page): Promise<void> {
	await expect(page.locator('[data-tab-strip-drag-ghost]')).toHaveCount(0);
	await expect(root(page).locator('[data-slot-animating]')).toHaveCount(0);
	await page.waitForFunction(
		() => {
			const testWindow = window as typeof window & {
				__tabStripStableLayout?: { count: number; signature: string };
			};
			const signature = [
				...document.querySelectorAll<HTMLElement>(
					'[data-horizontal-track] > [data-horizontal-item]',
				),
			]
				.map((element) =>
					[
						element.style.getPropertyValue('--ob-comp-tab-strip-inline-start'),
						element.style.getPropertyValue('--ob-comp-tab-strip-inline-size'),
						element.style.getPropertyValue('--ob-comp-tab-strip-runtime-opacity'),
					].join(':'),
				)
				.join('|');
			const previous = testWindow.__tabStripStableLayout;
			testWindow.__tabStripStableLayout =
				previous?.signature === signature
					? { signature, count: previous.count + 1 }
					: { signature, count: 1 };
			return testWindow.__tabStripStableLayout.count >= 4;
		},
		undefined,
		{ polling: 'raf' },
	);
}

async function collectionOrder(page: Page): Promise<readonly string[]> {
	return root(page)
		.locator('[data-horizontal-track] > [data-horizontal-item]')
		.evaluateAll((elements) =>
			elements.map((element) => {
				const groupId = element.querySelector<HTMLElement>('[data-group-id]')?.dataset.groupId;
				const tabId = element.querySelector<HTMLElement>('[data-tab-id]')?.dataset.tabId;
				if (groupId) return `group:${groupId}`;
				if (tabId) return `tab:${tabId}`;
				throw new Error('Unknown horizontal collection item.');
			}),
		);
}

async function paintedOrder(page: Page): Promise<readonly string[]> {
	return root(page)
		.locator('[data-horizontal-track] > [data-horizontal-item]')
		.evaluateAll((elements) =>
			elements
				.map((element) => {
					const groupId = element.querySelector<HTMLElement>('[data-group-id]')?.dataset.groupId;
					const tabId = element.querySelector<HTMLElement>('[data-tab-id]')?.dataset.tabId;
					return {
						id: groupId ? `group:${groupId}` : `tab:${tabId}`,
						left: Number.parseFloat(
							(element as HTMLElement).style.getPropertyValue('--ob-comp-tab-strip-inline-start'),
						),
					};
				})
				.sort((left, right) => left.left - right.left)
				.map(({ id }) => id),
		);
}

async function selectedTabIds(page: Page): Promise<readonly string[]> {
	return root(page)
		.locator('[data-tab-id][aria-selected="true"]')
		.evaluateAll((elements) =>
			elements.map((element) => {
				if (!(element instanceof HTMLElement) || !element.dataset.tabId) {
					throw new Error('Expected every selected tab to expose data-tab-id.');
				}
				return element.dataset.tabId;
			}),
		);
}

async function visibleActiveSurfaceIds(page: Page): Promise<readonly string[]> {
	return root(page)
		.locator('[data-horizontal-track] > [data-horizontal-item]')
		.evaluateAll((items) =>
			items.flatMap((item) => {
				const wrapper = item as HTMLElement;
				const activeGroup = wrapper.querySelector<HTMLElement>(
					'[data-tab-strip-group-header][data-active]',
				);
				if (
					activeGroup &&
					!wrapper.hasAttribute('data-focus-hidden') &&
					getComputedStyle(wrapper).visibility !== 'hidden'
				) {
					return [`group:${activeGroup.dataset.groupId}`];
				}
				const active = wrapper.querySelector<HTMLElement>('[data-tab-id][data-active]');
				if (
					!active ||
					wrapper.hasAttribute('data-group-collapsed-hidden') ||
					wrapper.hasAttribute('data-focus-hidden') ||
					getComputedStyle(wrapper).visibility === 'hidden'
				)
					return [];
				return [`tab:${active.dataset.tabId}`];
			}),
		);
}

test.beforeEach(async ({ page }) => {
	await page.goto('/');
	await expect(root(page)).toHaveAttribute('data-orientation', 'horizontal');
	await waitForSettledLayout(page);
	expect(await visibleActiveSurfaceIds(page)).toEqual(['tab:tabs-source']);
});

test('expanded group is dragged as one full visual and commits at a collection boundary', async ({
	page,
}) => {
	await beginDrag(page, sourceHeader(page));

	const ghost = page.locator('[data-tab-strip-drag-ghost][data-layout="horizontal"]');
	await expect(ghost).toHaveCount(1);
	await expect(ghost.locator('[data-group-id="research"]')).toHaveCount(1);
	await expect(ghost.locator('[data-tab-id="source"]')).toHaveCount(1);
	await expect(ghost.locator('[data-tab-id="tabs-source"]')).toHaveCount(1);

	await movePointerTo(page, tab(page, 'issue'));
	await page.mouse.up();
	await waitForSettledLayout(page);

	expect(await collectionOrder(page)).toEqual([
		'tab:mail',
		'tab:calendar',
		'group:build',
		'tab:docs',
		'tab:preview',
		'tab:issue',
		'tab:new-tab',
		'group:research',
		'tab:source',
		'tab:tabs-source',
	]);
	expect(await paintedOrder(page)).toEqual(await collectionOrder(page));
	await expect(sourceHeader(page)).toHaveAttribute('aria-expanded', 'true');
	expect(await selectedTabIds(page)).toEqual(['tabs-source']);
	expect(await visibleActiveSurfaceIds(page)).toEqual(['tab:tabs-source']);
	await expect(
		root(page).locator('[data-tab-strip-group-header][data-group-id="research"]'),
	).toHaveCount(1);
});

test('collapsed inactive group activates its first member but keeps a compact drag visual', async ({
	page,
}) => {
	await tab(page, 'issue').click();
	await sourceHeader(page).click();
	await expect(sourceHeader(page)).toHaveAttribute('aria-expanded', 'false');

	await beginDrag(page, sourceHeader(page));
	const ghost = page.locator('[data-tab-strip-drag-ghost][data-layout="horizontal"]');
	await expect(ghost.locator('[data-group-id="research"]')).toHaveCount(1);
	await expect(ghost.locator('[data-tab-id="source"]')).toHaveCount(0);
	await expect(ghost.locator('[data-tab-id="tabs-source"]')).toHaveCount(0);
	await expect(page.getByRole('heading', { level: 1 })).toHaveText('orchbee / orchbee');
	await expect(sourceHeader(page)).toHaveAttribute('aria-expanded', 'false');

	// The narrower product-only group header places the old target center on an
	// insertion threshold, so use the trailing tab as the deterministic boundary.
	await movePointerTo(page, tab(page, 'new-tab'));
	await page.mouse.up();
	await waitForSettledLayout(page);

	await expect(sourceHeader(page)).toHaveAttribute('aria-expanded', 'false');
	await expect(page.getByRole('heading', { level: 1 })).toHaveText('orchbee / orchbee');
	await expect(sourceHeader(page)).toHaveAttribute('data-active-tab-id', 'source');
	expect(await selectedTabIds(page)).toEqual(['source']);
	expect(await visibleActiveSurfaceIds(page)).toEqual(['group:research']);
	expect(await collectionOrder(page)).toEqual([
		'tab:mail',
		'tab:calendar',
		'group:build',
		'tab:docs',
		'tab:preview',
		'tab:issue',
		'tab:new-tab',
		'group:research',
		'tab:source',
		'tab:tabs-source',
	]);
	expect(await paintedOrder(page)).toEqual(await collectionOrder(page));
});

test('every stable collapse/expand projection exposes at least one active surface', async ({
	page,
}) => {
	expect(await visibleActiveSurfaceIds(page)).toEqual(['tab:tabs-source']);

	await sourceHeader(page).click();
	await waitForSettledLayout(page);
	await expect(sourceHeader(page)).toHaveAttribute('aria-expanded', 'false');
	await expect(sourceHeader(page)).toHaveAttribute('data-active-tab-id', 'tabs-source');
	await expect(sourceHeader(page).locator('[data-group-active-background]')).toHaveCount(1);
	await expect(sourceHeader(page)).toBeFocused();
	expect(await visibleActiveSurfaceIds(page)).toEqual(['group:research']);
	expect(await selectedTabIds(page)).toEqual(['tabs-source']);

	await groupHeader(page, 'build').click();
	await waitForSettledLayout(page);
	await expect(groupHeader(page, 'build')).toHaveAttribute('aria-expanded', 'false');
	expect(await visibleActiveSurfaceIds(page)).toEqual(['group:research']);
	expect(await selectedTabIds(page)).toEqual(['tabs-source']);

	await tab(page, 'issue').click();
	expect(await visibleActiveSurfaceIds(page)).toEqual(['tab:issue']);
	expect(await selectedTabIds(page)).toEqual(['issue']);

	await sourceHeader(page).click();
	await waitForSettledLayout(page);
	await expect(sourceHeader(page).locator('[data-group-active-background]')).toHaveCount(0);
	await tab(page, 'source').click();
	expect(await visibleActiveSurfaceIds(page)).toEqual(['tab:source']);
	expect(await selectedTabIds(page)).toEqual(['source']);
});

test('Escape restores the complete pre-drag observable state', async ({ page }) => {
	const initialOrder = await collectionOrder(page);
	await beginDrag(page, sourceHeader(page));
	await movePointerTo(page, tab(page, 'issue'));
	await page.keyboard.press('Escape');
	await page.mouse.up();
	await waitForSettledLayout(page);

	expect(await collectionOrder(page)).toEqual(initialOrder);
	expect(await paintedOrder(page)).toEqual(initialOrder);
	await expect(sourceHeader(page)).toHaveAttribute('aria-expanded', 'true');
	expect(await selectedTabIds(page)).toEqual(['tabs-source']);
	expect(await visibleActiveSurfaceIds(page)).toEqual(['tab:tabs-source']);
});
