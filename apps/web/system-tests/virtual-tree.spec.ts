import { createServer, type Server } from 'node:http';
import { type AddressInfo } from 'node:net';
import path from 'node:path';

import { createRsbuild, loadConfig, type RsbuildDevServer } from '@rsbuild/core';
import { expect, test, type Browser, type Page } from '@playwright/test';
import { z } from 'zod';

// The virtual tree's render benchmark and structure checks, on the page that
// `src/__tests__/virtual-tree.system-fixture.ts` builds. Structure is asserted; timings are measured
// in the page and printed, since they depend on the machine.

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
	measureExpand(): Promise<unknown>;
	measureScroll(pixelsPerFrame: number, frames: number): Promise<unknown>;
	measureSelectAll(): Promise<unknown>;
	scrollTop(): number;
	setFocus(key: string | null): void;
	setScrollTop(offset: number): void;
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
const scrollSchema = z.object({
	frameIntervalMs: statsSchema,
	frameMs: statsSchema,
	frames: z.number(),
	longTasks: z.number(),
	pixelsPerFrame: z.number(),
	scriptMs: statsSchema,
});
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

	const report = {
		expand: medians(expand),
		scroll,
		selectAll: medians(selectAll),
		userAgent: await page.evaluate(() => navigator.userAgent),
	};
	await test.info().attach('virtual-tree-render-benchmark', {
		body: JSON.stringify(report, null, 2),
		contentType: 'application/json',
	});
	process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
});

async function openFixture(page: Page): Promise<void> {
	if (fixtureUrl === undefined) throw new Error('The fixture server is unavailable.');
	await page.setViewportSize({ height: 800, width: 800 });
	await page.goto(fixtureUrl);
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
