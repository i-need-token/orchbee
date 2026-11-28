import { createServer, type Server } from 'node:http';
import { type AddressInfo } from 'node:net';
import path from 'node:path';

import { createRsbuild, loadConfig, type RsbuildDevServer } from '@rsbuild/core';
import { expect, test, type Browser, type Page } from '@playwright/test';
import { z } from 'zod';

// The virtual tree's render benchmark, structure checks and interaction checks, on the page that
// `src/__tests__/virtual-tree.system-fixture.ts` builds. Structure and interaction are asserted;
// timings are measured in the page and printed, since they depend on the machine.

const webRoot = path.resolve(import.meta.dirname, '..');
const fixtureEntry = 'virtual-tree-fixture';
const rowHeight = 22;
const treeHeight = 600;
const largeFolderRows = 100_003;

let developmentServer: RsbuildDevServer | undefined;
let fixtureUrl: string | undefined;

test.beforeAll(async () => {
	test.setTimeout(120_000);
	const started = await startFixtureServer();
	developmentServer = started.server;
	fixtureUrl = `${started.url}${fixtureEntry}`;
});

test.afterAll(async () => {
	await developmentServer?.close();
	developmentServer = undefined;
	fixtureUrl = undefined;
});

interface VirtualTreeFixture {
	collapse(key: string): void;
	measureDragStart(): Promise<unknown>;
	measureExpand(): Promise<unknown>;
	measureFind(mode: 'filter' | 'highlight', patterns: readonly string[]): Promise<unknown>;
	measureScroll(pixelsPerFrame: number, frames: number): Promise<unknown>;
	measureSelectAll(): Promise<unknown>;
	measureSelectSiblings(): Promise<unknown>;
	measureTypeNavigation(): Promise<unknown>;
	scrollTop(): number;
	setFocus(key: string | null): void;
	setScrollTop(offset: number): void;
	state(): unknown;
	takeDrops(): unknown;
	takeRequests(): unknown;
}

type FixtureWindow = Window & { readonly virtualTreeFixture: VirtualTreeFixture };

const statsSchema = z.object({ max: z.number(), median: z.number(), p95: z.number() });
const paintedChangeSchema = z.object({
	renderMs: z.number(),
	scriptMs: z.number(),
	toPaintMs: z.number(),
	workMs: z.number(),
});
const expandSchema = paintedChangeSchema.extend({ rowCount: z.number() });
const selectAllSchema = paintedChangeSchema.extend({ selected: z.number() });
const typeNavigationSchema = z.object({ rowCount: z.number(), scriptMs: z.number() });
const findKeystrokesSchema = z.array(
	z.object({
		appliedMs: z.number(),
		inputMs: z.number(),
		longestTaskMs: z.number(),
		matchCount: z.number(),
		pattern: z.string(),
		rowCount: z.number(),
		toPaintMs: z.number(),
	}),
);
const dragStartSchema = z.object({
	carried: z.number(),
	renderMs: z.number(),
	scriptMs: z.number(),
});
const scrollSchema = z.object({
	frameIntervalMs: statsSchema,
	frameMs: statsSchema,
	frames: z.number(),
	longTasks: z.number(),
	pixelsPerFrame: z.number(),
	scriptMs: statsSchema,
});
const treeStateSchema = z.object({
	anchor: z.string().nullable(),
	focus: z.string().nullable(),
	rowCount: z.number(),
	selection: z.array(z.string()),
});
const treeRequestsSchema = z.array(
	z.object({
		key: z.string().nullable(),
		kind: z.enum(['contextMenu', 'open']),
		offset: z.object({ x: z.number(), y: z.number() }).optional(),
		pinned: z.boolean().optional(),
		preserveFocus: z.boolean().optional(),
		segment: z.string().nullable().optional(),
		sideBySide: z.boolean().optional(),
	}),
);
const treeDropsSchema = z.array(
	z.object({
		ctrlKey: z.boolean(),
		keys: z.array(z.string()),
		target: z.string().nullable(),
	}),
);
const traceEventSchema = z.object({
	dur: z.number().optional(),
	name: z.string(),
	ph: z.string(),
	pid: z.number(),
	tid: z.number(),
	ts: z.number(),
});
const traceSchema = z.object({ traceEvents: z.array(z.unknown()) });

/** Main-thread events that run script; a layout inside one is one that script forced. */
const scriptEvents = new Set([
	'EvaluateScript',
	'EventDispatch',
	'FireAnimationFrame',
	'FunctionCall',
	'RunMicrotasks',
	'TimerFire',
]);

test('virtual tree renders only the rows in sight and the focused row', async ({ page }) => {
	await openFixture(page);
	expect(await rowStructure(page)).toMatchObject({ inSight: 3, outOfSightFocused: 0, others: 0 });

	const expanded = expandSchema.parse(
		await page.evaluate(() =>
			(window as unknown as FixtureWindow).virtualTreeFixture.measureExpand(),
		),
	);
	expect(expanded.rowCount).toBe(largeFolderRows);
	expect(await rowStructure(page)).toEqual({
		activeDescendant: null,
		covered: true,
		gaps: 0,
		inSight: Math.ceil(treeHeight / rowHeight),
		outOfSightFocused: 0,
		others: 0,
	});

	// The focused row keeps its element while it is scrolled away, so the active descendant resolves.
	await page.evaluate(() => {
		const fixture = (window as unknown as FixtureWindow).virtualTreeFixture;
		fixture.setFocus('folder');
		fixture.setScrollTop(50_000);
	});
	await nextFrame(page);
	expect(await rowStructure(page)).toMatchObject({
		activeDescendant: { level: '1', position: '1' },
		covered: true,
		gaps: 0,
		outOfSightFocused: 1,
		others: 0,
	});
	const frame = page.getByRole('tree');
	await expect(frame).toHaveAttribute('data-overflowing', '');
	await expect(frame).toHaveAttribute('data-scrolled-start', '');

	// A wheel notch scrolls VS Code's 50 px, Alt five times as far, Shift outside macOS not at all.
	const box = await frame.boundingBox();
	if (box === null) throw new Error('The tree has no box.');
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	expect(await wheelScroll(page, null)).toBe(50);
	expect(await wheelScroll(page, 'Alt')).toBe(250);
	if (process.platform !== 'darwin') expect(await wheelScroll(page, 'Shift')).toBe(0);
});

test('virtual tree answers keys, clicks and the context menu as VS Code does', async ({ page }) => {
	await openFixture(page);
	const tree = page.getByRole('tree');
	const row = (name: string) => page.getByRole('treeitem', { exact: true, name });
	const state = async () =>
		treeStateSchema.parse(
			await page.evaluate(() => (window as unknown as FixtureWindow).virtualTreeFixture.state()),
		);
	const requests = async () =>
		treeRequestsSchema.parse(
			await page.evaluate(() =>
				(window as unknown as FixtureWindow).virtualTreeFixture.takeRequests(),
			),
		);
	const preview = (key: string) => ({
		key,
		kind: 'open',
		pinned: false,
		preserveFocus: true,
		sideBySide: false,
	});
	const box = await tree.boundingBox();
	if (box === null) throw new Error('The tree has no box.');
	const emptySpace = { x: box.x + box.width / 2, y: box.y + box.height / 2 };

	// A click focuses the tree, then focuses, anchors and selects its row and previews it.
	await row('package.json').click();
	await expect(tree).toBeFocused();
	expect(await state()).toEqual({
		anchor: 'package.json',
		focus: 'package.json',
		rowCount: 3,
		selection: ['package.json'],
	});
	expect(await requests()).toEqual([preview('package.json')]);

	// Down moves focus and the anchor, and the focus ring shows; Enter selects and opens for good.
	await page.keyboard.press('ArrowDown');
	expect(await state()).toMatchObject({ anchor: 'README.md', focus: 'README.md' });
	await expect(tree).toHaveAttribute(
		'aria-activedescendant',
		(await row('README.md').getAttribute('id')) ?? '',
	);
	expect(await tree.evaluate((element) => element.matches(':focus-visible'))).toBe(true);
	await page.keyboard.press('Enter');
	expect(await state()).toMatchObject({ selection: ['README.md'] });
	expect(await requests()).toEqual([
		{ key: 'README.md', kind: 'open', pinned: true, preserveFocus: false, sideBySide: false },
	]);

	// Shift+F10 and the context menu key each ask once for the focused row's menu, at its corner.
	await page.keyboard.press('Shift+F10');
	await page.keyboard.press('ContextMenu');
	const keyMenu = {
		key: 'README.md',
		kind: 'contextMenu',
		offset: { x: 0, y: 0 },
		segment: 'README.md',
	};
	expect(await requests()).toEqual([keyMenu, keyMenu]);

	// A right click focuses its row before asking; on the empty space it clears focus.
	await row('folder').click({ button: 'right' });
	expect(await state()).toMatchObject({ focus: 'folder' });
	await page.mouse.click(emptySpace.x, emptySpace.y, { button: 'right' });
	expect(await state()).toMatchObject({ focus: null, selection: ['README.md'] });
	expect((await requests()).map((request) => request.key)).toEqual(['folder', null]);

	// Shift-click selects from the anchor; Ctrl-click, Cmd-click on macOS, takes one row out.
	await row('package.json').click({ modifiers: ['Shift'] });
	expect(await state()).toMatchObject({
		anchor: 'README.md',
		focus: 'package.json',
		selection: ['README.md', 'package.json'],
	});
	await row('README.md').click({ modifiers: ['ControlOrMeta'] });
	expect(await state()).toMatchObject({
		anchor: 'README.md',
		focus: 'README.md',
		selection: ['package.json'],
	});
	await page.keyboard.press('Escape');
	expect(await state()).toMatchObject({ anchor: null, focus: null, selection: [] });
	expect(await requests()).toEqual([]);

	// Typing moves focus to the next row its label starts, wrapping once a word has ended.
	await page.keyboard.press('r');
	expect(await state()).toMatchObject({ focus: 'README.md', selection: [] });
	await page.waitForTimeout(900);
	await page.keyboard.press('f');
	expect(await state()).toMatchObject({ focus: 'folder' });

	// Space expands and collapses a folder without opening it, and previews a file.
	await page.keyboard.press('Space');
	await expect.poll(async () => (await state()).rowCount).toBe(largeFolderRows);
	await page.keyboard.press('Space');
	expect(await state()).toMatchObject({ rowCount: 3 });
	await page.keyboard.press('End');
	await page.keyboard.press('Space');
	expect(await state()).toMatchObject({ focus: 'README.md', selection: ['README.md'] });
	expect(await requests()).toEqual([preview('README.md')]);

	// A double click previews on its first click, then opens for good and takes focus.
	await row('package.json').dblclick();
	expect(await requests()).toEqual([
		preview('package.json'),
		{ key: 'package.json', kind: 'open', pinned: true, preserveFocus: false, sideBySide: false },
	]);
	// A middle click opens for good and keeps focus.
	await row('README.md').click({ button: 'middle' });
	expect(await state()).toMatchObject({ focus: 'README.md', selection: ['README.md'] });
	expect(await requests()).toEqual([
		{ key: 'README.md', kind: 'open', pinned: true, preserveFocus: true, sideBySide: false },
	]);

	// The twistie only toggles; a click elsewhere on a folder's row toggles it and selects it.
	await row('folder').locator('[data-twistie]').click();
	await expect.poll(async () => (await state()).rowCount).toBe(largeFolderRows);
	expect(await state()).toMatchObject({ focus: 'folder', selection: ['README.md'] });
	await row('folder').click();
	expect(await state()).toMatchObject({ rowCount: 3, selection: ['folder'] });
	expect(await requests()).toEqual([preview('folder')]);

	// A click on the empty space clears focus, selection and anchor.
	await page.mouse.click(emptySpace.x, emptySpace.y);
	expect(await state()).toEqual({ anchor: null, focus: null, rowCount: 3, selection: [] });
});

test('virtual tree finds rows as VS Code’s tree find does', async ({ page }) => {
	await openFixture(page);
	const tree = page.getByRole('tree');
	const find = page.getByRole('search', { name: 'Find' });
	const input = find.getByRole('textbox', { name: 'Type to search' });
	const announced = page.getByRole('status');
	const state = async () =>
		treeStateSchema.parse(
			await page.evaluate(() => (window as unknown as FixtureWindow).virtualTreeFixture.state()),
		);
	const highlighted = (name: string) =>
		page.getByRole('treeitem', { exact: true, name }).locator('[class*="highlight"]');

	// Ctrl+Alt+F, Cmd+Alt+F on macOS, opens the find above the rows and focuses its input.
	await page.getByRole('treeitem', { exact: true, name: 'package.json' }).click();
	await page.keyboard.press('ControlOrMeta+Alt+KeyF');
	await expect(input).toBeFocused();
	await expect(find).toMatchAriaSnapshot(`
		- search "Find":
		  - textbox "Type to search"
		  - button "Filter"
		  - button "Fuzzy Match" [pressed]
		  - button "Close"
	`);

	// Typing moves focus to the first match, highlights what matched and announces the count.
	await page.keyboard.type('READ');
	await expect.poll(async () => (await state()).focus).toBe('README.md');
	await expect(highlighted('README.md')).toHaveText(['READ']);
	await expect(announced).toHaveText('1 result');

	// Filter keeps the matched rows alone, and a click on it leaves focus in the input.
	await find.getByRole('button', { name: 'Filter' }).click();
	await expect.poll(async () => (await state()).rowCount).toBe(1);
	await expect(input).toBeFocused();
	await expect(input).toHaveAttribute('placeholder', 'Type to filter');
	await page.keyboard.type('X');
	await expect(find.getByText('No results found.')).toBeVisible();
	await expect(announced).toHaveText('No results');

	// Escape shows every row again and returns focus to the tree.
	await page.keyboard.press('Escape');
	await expect(find).toBeHidden();
	await expect(tree).toBeFocused();
	expect(await state()).toMatchObject({ focus: 'README.md', rowCount: 3 });

	// F3 opens it again with the pattern it closed with, selected, still filtering; Enter returns
	// to the tree and leaves it open.
	await page.keyboard.press('F3');
	await expect(input).toHaveValue('READX');
	expect(
		await input.evaluate((element) =>
			element instanceof HTMLInputElement ? [element.selectionStart, element.selectionEnd] : null,
		),
	).toEqual([0, 5]);
	await page.keyboard.press('End');
	await page.keyboard.press('Backspace');
	await expect.poll(async () => (await state()).rowCount).toBe(1);
	await page.keyboard.press('Enter');
	await expect(tree).toBeFocused();
	await expect(find).toBeVisible();
});

test('virtual tree drags rows and takes drops as VS Code does', async ({ page }) => {
	await openFixture(page);
	const tree = page.getByRole('tree');
	const row = (name: string) => page.getByRole('treeitem', { exact: true, name });
	const label = page.locator('body > div[aria-hidden="true"]');
	const state = async () =>
		treeStateSchema.parse(
			await page.evaluate(() => (window as unknown as FixtureWindow).virtualTreeFixture.state()),
		);
	const requests = async () =>
		treeRequestsSchema.parse(
			await page.evaluate(() =>
				(window as unknown as FixtureWindow).virtualTreeFixture.takeRequests(),
			),
		);
	const drops = async () =>
		treeDropsSchema.parse(
			await page.evaluate(() =>
				(window as unknown as FixtureWindow).virtualTreeFixture.takeDrops(),
			),
		);
	const scrollTop = () =>
		page.evaluate(() => (window as unknown as FixtureWindow).virtualTreeFixture.scrollTop());
	const lit = () => page.locator('[role="treeitem"][data-drop-target]').allTextContents();
	const cursor = () => tree.evaluate((element) => getComputedStyle(element).cursor);
	const center = async (name: string) => {
		const box = await row(name).boundingBox();
		if (box === null) throw new Error(`${name} has no box.`);
		return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
	};
	const box = await tree.boundingBox();
	if (box === null) throw new Error('The tree has no box.');

	// A press that travels no farther than 10 px stays a click.
	const readme = await center('README.md');
	await page.mouse.move(readme.x, readme.y);
	await page.mouse.down();
	await page.mouse.move(readme.x + 6, readme.y + 8);
	await page.mouse.up();
	expect(await state()).toMatchObject({ focus: 'README.md', selection: ['README.md'] });
	expect(await requests()).toHaveLength(1);

	// Farther, an unselected row drags alone and leaves the selection: its label follows the pointer
	// 10 px off, and every element shows the grabbing cursor.
	const packageJson = await center('package.json');
	await page.mouse.move(packageJson.x, packageJson.y);
	await page.mouse.down();
	await page.mouse.move(packageJson.x + 20, packageJson.y, { steps: 2 });
	await expect(label).toHaveText('package.json');
	const labelBox = await label.boundingBox();
	expect(Math.abs((labelBox?.x ?? 0) - (packageJson.x + 30))).toBeLessThanOrEqual(1);
	expect(Math.abs((labelBox?.y ?? 0) - (packageJson.y + 10))).toBeLessThanOrEqual(1);
	expect(await cursor()).toBe('grabbing');

	// A top-level file hands the drop to the whole tree, and a row never takes itself. The drag holds
	// the pointer, so no row shows hover and only a drop lights one.
	await page.mouse.move(readme.x, readme.y, { steps: 2 });
	await expect(tree).toHaveAttribute('data-drop-target', '');
	expect(await lit()).toEqual([]);
	await page.mouse.move(packageJson.x, packageJson.y, { steps: 2 });
	await expect(tree).not.toHaveAttribute('data-drop-target');
	expect(await lit()).toEqual([]);
	expect(await row('package.json').evaluate((element) => element.matches(':hover'))).toBe(false);

	// A folder lights up, and once the drag rests on it for 500 ms it expands with its rows lit.
	const folder = await center('folder');
	const arrived = Date.now();
	await page.mouse.move(folder.x, folder.y, { steps: 2 });
	await expect.poll(lit).toEqual(['folder']);
	await expect.poll(async () => (await state()).rowCount).toBe(largeFolderRows);
	expect(Date.now() - arrived).toBeGreaterThanOrEqual(500);
	await expect.poll(async () => (await lit()).length).toBe(Math.ceil(treeHeight / rowHeight));

	// The release drops with the keys held, and no click follows it.
	await page.keyboard.down('Control');
	await page.mouse.up();
	await page.keyboard.up('Control');
	expect(await drops()).toEqual([{ ctrlKey: true, keys: ['package.json'], target: 'folder' }]);
	await expect(label).toHaveCount(0);
	expect(await lit()).toEqual([]);
	expect(await cursor()).not.toBe('grabbing');
	expect(await state()).toMatchObject({ focus: 'README.md', selection: ['README.md'] });
	expect(await requests()).toEqual([]);

	// Within 35 px of an edge the tree scrolls while the drag rests there; Esc cancels it, and its
	// release clicks nothing.
	const file = await center('file-1.ts');
	await page.mouse.move(file.x, file.y);
	await page.mouse.down();
	await page.mouse.move(file.x, box.y + box.height - 5, { steps: 4 });
	await expect.poll(scrollTop).toBeGreaterThan(200);
	const scrolled = await scrollTop();
	await page.mouse.move(file.x, box.y + 5, { steps: 4 });
	await expect.poll(scrollTop).toBeLessThan(scrolled - 100);
	await page.keyboard.press('Escape');
	await expect(label).toHaveCount(0);
	expect(await lit()).toEqual([]);
	await page.mouse.up();
	expect(await drops()).toEqual([]);
	expect(await state()).toMatchObject({ focus: 'README.md', selection: ['README.md'] });
	expect(await requests()).toEqual([]);

	// A selected row carries the selection in tree order, and the empty space drops on the tree.
	await page.evaluate(() => {
		(window as unknown as FixtureWindow).virtualTreeFixture.collapse('folder');
	});
	await row('package.json').click();
	await row('README.md').click({ modifiers: ['Shift'] });
	await requests();
	const selected = await center('README.md');
	await page.mouse.move(selected.x, selected.y);
	await page.mouse.down();
	await page.mouse.move(selected.x, box.y + box.height / 2, { steps: 4 });
	await expect(label).toHaveText('2');
	await expect(tree).toHaveAttribute('data-drop-target', '');
	await page.mouse.up();
	expect(await drops()).toEqual([
		{ ctrlKey: false, keys: ['package.json', 'README.md'], target: null },
	]);
	expect(await state()).toMatchObject({ selection: ['README.md', 'package.json'] });
	expect(await requests()).toEqual([]);
});

test('virtual tree joins folders into compact rows and acts on their items as VS Code does', async ({
	page,
}) => {
	await openFixture(page, 'compact');
	const tree = page.getByRole('tree');
	const label = page.locator('body > div[aria-hidden="true"]');
	const state = async () =>
		treeStateSchema.parse(
			await page.evaluate(() => (window as unknown as FixtureWindow).virtualTreeFixture.state()),
		);
	const requests = async () =>
		treeRequestsSchema.parse(
			await page.evaluate(() =>
				(window as unknown as FixtureWindow).virtualTreeFixture.takeRequests(),
			),
		);
	const drops = async () =>
		treeDropsSchema.parse(
			await page.evaluate(() =>
				(window as unknown as FixtureWindow).virtualTreeFixture.takeDrops(),
			),
		);
	const compactRow = page.locator('[data-row-index="0"]');
	const items = compactRow.locator('[data-tree-segment]');
	const itemName = (index: number) => items.nth(index).locator('span').first();
	const center = async (locator: ReturnType<Page['locator']>) => {
		const box = await locator.boundingBox();
		if (box === null) throw new Error('The element has no box.');
		return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
	};
	const rowId = (await compactRow.getAttribute('id')) ?? '';

	// Folders that each hold one folder share a row. Each item is a tree item of its own, at its own
	// level and announced with its parents in the row, while the rows below indent once for the row.
	await expect.poll(async () => (await state()).rowCount).toBe(4);
	await expect(items).toHaveText(['src/', 'app/', 'lib']);
	expect(
		await items.evaluateAll((elements) =>
			elements.map((element) => ({
				expanded: element.getAttribute('aria-expanded'),
				id: element.id,
				label: element.getAttribute('aria-label'),
				level: element.getAttribute('aria-level'),
				position: element.getAttribute('aria-posinset'),
				role: element.getAttribute('role'),
				size: element.getAttribute('aria-setsize'),
			})),
		),
	).toEqual([
		{
			expanded: 'true',
			id: `${rowId}-0`,
			label: 'src',
			level: '1',
			position: '1',
			role: 'treeitem',
			size: '2',
		},
		{
			expanded: 'true',
			id: `${rowId}-1`,
			label: 'app, compact, src',
			level: '2',
			position: '1',
			role: 'treeitem',
			size: '1',
		},
		{
			expanded: 'true',
			id: `${rowId}-2`,
			label: 'lib, compact, app, src',
			level: '3',
			position: '1',
			role: 'treeitem',
			size: '1',
		},
	]);
	// The row itself is named by its last item, as VS Code's Explorer names it.
	await expect(compactRow).toHaveAccessibleName('lib, compact, app, src');
	const firstFile = page.locator('[data-row-index="1"]');
	await expect(firstFile).toHaveAttribute('aria-level', '4');
	await expect(firstFile).toHaveCSS('--ob-comp-virtual-tree-row-depth', '2');

	// A click on an item's name focuses that item, toggles, selects and previews the row, and the
	// tree names the item as its active descendant.
	await itemName(1).click();
	await expect(tree).toBeFocused();
	expect(await state()).toEqual({
		anchor: 'src/app/lib',
		focus: 'src/app',
		rowCount: 2,
		selection: ['src/app/lib'],
	});
	await expect(tree).toHaveAttribute('aria-activedescendant', `${rowId}-1`);
	await expect(items.nth(1)).toHaveAttribute('data-active', '');
	await expect(items.nth(0)).toHaveAttribute('aria-expanded', 'false');
	expect(await requests()).toEqual([
		{ key: 'src/app/lib', kind: 'open', pinned: false, preserveFocus: true, sideBySide: false },
	]);

	// Left, Right, Home and End move among the items first; on the first or the last item, they
	// collapse, expand or move focus as on any row.
	await page.keyboard.press('ArrowLeft');
	expect(await state()).toMatchObject({ focus: 'src', rowCount: 2 });
	await page.keyboard.press('ArrowRight');
	expect(await state()).toMatchObject({ focus: 'src/app', rowCount: 2 });
	await page.keyboard.press('End');
	await page.keyboard.press('ArrowRight');
	expect(await state()).toMatchObject({ focus: 'src/app/lib', rowCount: 4 });
	await page.keyboard.press('Home');
	await page.keyboard.press('ArrowLeft');
	expect(await state()).toMatchObject({ focus: 'src', rowCount: 2 });
	await expect(tree).toHaveAttribute('aria-activedescendant', `${rowId}-0`);
	await page.keyboard.press('End');
	await page.keyboard.press('End');
	expect(await state()).toMatchObject({ focus: 'package.json' });
	await page.keyboard.press('Home');
	expect(await state()).toMatchObject({ focus: 'src/app/lib', selection: ['src/app/lib'] });
	await expect(tree).toHaveAttribute('aria-activedescendant', `${rowId}-2`);

	// The keyboard asks for the focused item's menu at its name; a right click asks for the item
	// whose name it lands on, and elsewhere on the row for the row's own item.
	await page.keyboard.press('ArrowLeft');
	await page.keyboard.press('Shift+F10');
	expect(await requests()).toEqual([
		{ key: 'src/app/lib', kind: 'contextMenu', offset: { x: 0, y: 0 }, segment: 'src/app' },
	]);
	await itemName(0).click({ button: 'right' });
	expect(await state()).toMatchObject({ focus: 'src' });
	await compactRow.locator('[data-twistie]').click({ button: 'right' });
	expect(await state()).toMatchObject({ focus: 'src/app/lib' });
	expect((await requests()).map((request) => request.segment)).toEqual(['src', 'src/app/lib']);

	// A drag over an item's name lights that item alone and drops on it, and resting there for
	// 500 ms opens the row.
	const packageJson = await center(page.locator('[data-row-index="1"]'));
	await page.mouse.move(packageJson.x, packageJson.y);
	await page.mouse.down();
	await page.mouse.move(packageJson.x + 20, packageJson.y, { steps: 2 });
	const app = await center(itemName(1));
	await page.mouse.move(app.x, app.y, { steps: 2 });
	await expect(items.nth(1)).toHaveAttribute('data-drop-target', '');
	expect(await page.locator('[data-row-index][data-drop-target]').count()).toBe(0);
	await expect.poll(async () => (await state()).rowCount).toBe(4);
	await page.mouse.up();
	expect(await drops()).toEqual([{ ctrlKey: false, keys: ['package.json'], target: 'src/app' }]);
	await expect(items.nth(1)).not.toHaveAttribute('data-drop-target');

	// A drag that takes the row alone carries the item whose name it started on.
	const src = await center(itemName(0));
	const box = await tree.boundingBox();
	if (box === null) throw new Error('The tree has no box.');
	await page.mouse.move(src.x, src.y);
	await page.mouse.down();
	await page.mouse.move(src.x, box.y + box.height / 2, { steps: 4 });
	await expect(label).toHaveText('src');
	await page.mouse.up();
	expect(await drops()).toEqual([{ ctrlKey: false, keys: ['src'], target: null }]);
	expect(await requests()).toEqual([]);

	// The find highlights the characters each name matched, and never a separator.
	await tree.focus();
	await page.keyboard.press('ControlOrMeta+Alt+KeyF');
	await page.keyboard.type('p/l');
	await expect(items.nth(1).locator('[class*="highlight"]')).toHaveText(['p']);
	await expect(items.nth(2).locator('[class*="highlight"]')).toHaveText(['l']);
	await expect(items.nth(0).locator('[class*="highlight"]')).toHaveCount(0);
});

test('virtual tree render benchmark', async ({ browser, page }) => {
	test.setTimeout(180_000);
	await openFixture(page);
	const expand = await repeat(() =>
		page
			.evaluate(() => (window as unknown as FixtureWindow).virtualTreeFixture.measureExpand())
			.then((result) => expandSchema.parse(result)),
	);
	// One row, ten rows and fifty rows a frame.
	const scroll = await sequence([rowHeight, rowHeight * 10, rowHeight * 50], (pixelsPerFrame) =>
		traced(browser, page, () =>
			page
				.evaluate(
					(speed) =>
						(window as unknown as FixtureWindow).virtualTreeFixture.measureScroll(speed, 120),
					pixelsPerFrame,
				)
				.then((result) => scrollSchema.parse(result)),
		),
	);
	expect(scroll.map((measured) => measured.forcedLayouts)).toEqual([0, 0, 0]);
	const selectAll = await repeat(() =>
		page
			.evaluate(() => (window as unknown as FixtureWindow).virtualTreeFixture.measureSelectAll())
			.then((result) => selectAllSchema.parse(result)),
	);
	expect(selectAll.every((run) => run.selected === largeFolderRows)).toBe(true);
	// The select-all key on one of the folder's files selects its 100,000 siblings.
	const selectSiblings = await repeat(() =>
		page
			.evaluate(() =>
				(window as unknown as FixtureWindow).virtualTreeFixture.measureSelectSiblings(),
			)
			.then((result) => selectAllSchema.parse(result)),
	);
	expect(selectSiblings.every((run) => run.selected === largeFolderRows - 3)).toBe(true);
	const typeNavigation = await repeat(() =>
		page
			.evaluate(() =>
				(window as unknown as FixtureWindow).virtualTreeFixture.measureTypeNavigation(),
			)
			.then((result) => typeNavigationSchema.parse(result)),
	);
	// A drag from one of the 100,003 selected rows carries every one of them.
	const dragStart = await repeat(() =>
		page
			.evaluate(() => (window as unknown as FixtureWindow).virtualTreeFixture.measureDragStart())
			.then((result) => dragStartSchema.parse(result)),
	);
	expect(dragStart.every((run) => run.carried === largeFolderRows)).toBe(true);
	// Typing into the find over the expanded folder: every file matches `f`, and `9` matches 40,951.
	const findHighlight = await repeat(() =>
		page
			.evaluate(() =>
				(window as unknown as FixtureWindow).virtualTreeFixture.measureFind('highlight', [
					'f',
					'fi',
					'',
				]),
			)
			.then((result) => findKeystrokesSchema.parse(result)),
	);
	const findFilter = await repeat(() =>
		page
			.evaluate(() =>
				(window as unknown as FixtureWindow).virtualTreeFixture.measureFind('filter', [
					'9',
					'99',
					'',
				]),
			)
			.then((result) => findKeystrokesSchema.parse(result)),
	);
	expect(findFilter.every(([nine]) => nine?.matchCount === 40_951)).toBe(true);

	const report = {
		dragStart: medians(dragStart),
		expand: medians(expand),
		find: {
			filter: keystrokeMedians(findFilter),
			highlight: keystrokeMedians(findHighlight),
		},
		scroll,
		selectAll: medians(selectAll),
		selectSiblings: medians(selectSiblings),
		typeNavigation: medians(typeNavigation),
		userAgent: await page.evaluate(() => navigator.userAgent),
	};
	await test.info().attach('virtual-tree-render-benchmark', {
		body: JSON.stringify(report, null, 2),
		contentType: 'application/json',
	});
	process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
});

/** Opens the fixture's page: the benchmark tree, or the tree of compact rows. */
async function openFixture(page: Page, tree: 'compact' | null = null): Promise<void> {
	if (fixtureUrl === undefined) throw new Error('The fixture server is unavailable.');
	await page.setViewportSize({ height: 800, width: 800 });
	await page.goto(tree === null ? fixtureUrl : `${fixtureUrl}?tree=${tree}`);
	await expect(page.getByRole('treeitem').first()).toBeVisible();
}

/** Runs one measurement after another, since they share the page. */
async function sequence<Input, Result>(
	inputs: readonly Input[],
	run: (input: Input) => Promise<Result>,
): Promise<Result[]> {
	const results: Result[] = [];
	await inputs.reduce(
		(previous, input) =>
			previous.then(async () => {
				results.push(await run(input));
			}),
		Promise.resolve(),
	);
	return results;
}

/** Two warm-up runs, then seven measured ones, as the model benchmarks run. */
async function repeat<Result>(run: () => Promise<Result>): Promise<Result[]> {
	const results = await sequence(
		Array.from({ length: 9 }, (_, attempt) => attempt),
		run,
	);
	return results.slice(2);
}

/** Each keystroke's medians over the runs, keyed by the pattern it typed. */
function keystrokeMedians(runs: readonly z.infer<typeof findKeystrokesSchema>[]) {
	const keystrokes = new Map<string, Record<string, number>[]>();
	for (const run of runs) {
		for (const { pattern, ...timings } of run) {
			const key = pattern === '' ? '(empty)' : pattern;
			keystrokes.set(key, [...(keystrokes.get(key) ?? []), timings]);
		}
	}
	return Object.fromEntries(
		Array.from(keystrokes, ([pattern, timings]) => [pattern, medians(timings)]),
	);
}

function medians<Result extends Record<string, number>>(runs: readonly Result[]) {
	const median: Record<string, number> = {};
	for (const key of Object.keys(runs[0] ?? {})) {
		const values = runs.map((run) => run[key] ?? 0).sort((left, right) => left - right);
		median[key] = values[Math.floor(values.length / 2)] ?? 0;
	}
	return median;
}

async function nextFrame(page: Page): Promise<void> {
	await page.evaluate(
		() =>
			new Promise<void>((resolve) => {
				requestAnimationFrame(() => {
					requestAnimationFrame(() => {
						resolve();
					});
				});
			}),
	);
}

/** The scroll offset one wheel notch adds with a modifier held. */
async function wheelScroll(page: Page, modifier: 'Alt' | 'Shift' | null): Promise<number> {
	await page.evaluate(() => {
		(window as unknown as FixtureWindow).virtualTreeFixture.setScrollTop(50_000);
	});
	await nextFrame(page);
	if (modifier !== null) await page.keyboard.down(modifier);
	await page.mouse.wheel(0, 120);
	if (modifier !== null) await page.keyboard.up(modifier);
	await nextFrame(page);
	return (
		(await page.evaluate(() =>
			(window as unknown as FixtureWindow).virtualTreeFixture.scrollTop(),
		)) - 50_000
	);
}

/** Runs a measurement under a Chromium trace and counts the layouts that script forced. */
async function traced<Result>(
	browser: Browser,
	page: Page,
	run: () => Promise<Result>,
): Promise<Result & { readonly forcedLayouts: number; readonly layouts: number }> {
	await browser.startTracing(page, { categories: ['devtools.timeline'] });
	const result = await run();
	const trace = traceSchema.parse(JSON.parse((await browser.stopTracing()).toString('utf8')));
	const events = trace.traceEvents.flatMap((event) => {
		const parsed = traceEventSchema.safeParse(event);
		return parsed.success && parsed.data.ph === 'X' ? [parsed.data] : [];
	});
	const scripts = events.filter((event) => scriptEvents.has(event.name));
	const layouts = events.filter((event) => event.name === 'Layout');
	const forcedLayouts = layouts.filter((layout) =>
		scripts.some(
			(script) =>
				script.pid === layout.pid &&
				script.tid === layout.tid &&
				script.ts <= layout.ts &&
				layout.ts + (layout.dur ?? 0) <= script.ts + (script.dur ?? 0),
		),
	).length;
	return { ...result, forcedLayouts, layouts: layouts.length };
}

/** Which row elements exist, against the tree's box. */
async function rowStructure(page: Page) {
	return page.evaluate(() => {
		const tree = document.querySelector('[role="tree"]');
		if (!(tree instanceof HTMLElement)) throw new Error('The tree is missing.');
		const frame = tree.getBoundingClientRect();
		const rows = Array.from(tree.querySelectorAll('[role="treeitem"]'), (row) => {
			const box = row.getBoundingClientRect();
			return { bottom: box.bottom, focused: row.hasAttribute('data-focused'), top: box.top };
		});
		const inSight = rows
			.filter((row) => row.bottom > frame.top && row.top < frame.bottom)
			.sort((left, right) => left.top - right.top);
		const outOfSight = rows.filter((row) => !inSight.includes(row));
		const first = inSight[0];
		const last = inSight.at(-1);
		const active = tree.getAttribute('aria-activedescendant');
		const activeRow = active === null ? null : document.getElementById(active);
		return {
			activeDescendant:
				activeRow === null
					? null
					: {
							level: activeRow.getAttribute('aria-level'),
							position: activeRow.getAttribute('aria-posinset'),
						},
			covered:
				first !== undefined &&
				last !== undefined &&
				first.top <= frame.top &&
				last.bottom >= frame.bottom,
			gaps: inSight.filter(
				(row, index) => index > 0 && Math.abs(row.top - (inSight[index - 1]?.bottom ?? 0)) > 0.5,
			).length,
			inSight: inSight.length,
			outOfSightFocused: outOfSight.filter((row) => row.focused).length,
			others: outOfSight.filter((row) => !row.focused).length,
		};
	});
}

async function startFixtureServer(): Promise<{
	readonly server: RsbuildDevServer;
	readonly url: string;
}> {
	const port = await reserveLoopbackPort();
	const loaded = await loadConfig({ command: 'dev', cwd: webRoot });
	const rsbuild = await createRsbuild({
		callerName: 'orchbee-web-virtual-tree-system-test',
		cwd: webRoot,
		rsbuildConfig: {
			...loaded.content,
			server: { ...loaded.content.server, host: '127.0.0.1', port, strictPort: true },
			source: {
				...loaded.content.source,
				entry: { [fixtureEntry]: './src/__tests__/virtual-tree.system-fixture.ts' },
			},
		},
	});
	const initialCompilation = new Promise<Error | undefined>((resolve) => {
		rsbuild.onAfterDevCompile(({ isFirstCompile, stats }) => {
			if (!isFirstCompile) return;
			resolve(
				stats.hasErrors()
					? new Error(
							`Fixture compilation failed.\n${stats.toString({ all: false, errorDetails: true, errors: true })}`,
						)
					: undefined,
			);
		});
	});
	const started = await rsbuild.startDevServer({ getPortSilently: true });
	const compilationError = await initialCompilation;
	if (compilationError !== undefined) {
		await started.server.close();
		throw compilationError;
	}
	return { server: started.server, url: `http://127.0.0.1:${started.port}/` };
}

async function reserveLoopbackPort(): Promise<number> {
	const server = createServer();
	await new Promise<void>((resolve, reject) => {
		server.once('error', reject);
		server.listen(0, '127.0.0.1', () => {
			server.off('error', reject);
			resolve();
		});
	});
	const address = server.address() as AddressInfo | null;
	await closeLoopbackServer(server);
	if (address === null) throw new Error('The loopback reservation has no address.');
	return address.port;
}

async function closeLoopbackServer(server: Server): Promise<void> {
	await new Promise<void>((resolve, reject) => {
		server.close((error) => (error === undefined ? resolve() : reject(error)));
	});
}
