import { mkdtempSync, readFileSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it, vi } from 'vitest';

import { prepareDesktopDataRoot } from '../prepareDesktopDataRoot';

type DataRootDependencies = NonNullable<Parameters<typeof prepareDesktopDataRoot>[1]>;

const temporaryDirectories: string[] = [];

afterEach(() => {
	for (const directory of temporaryDirectories.splice(0)) {
		rmSync(directory, { force: true, recursive: true });
	}
});

describe('prepareDesktopDataRoot', () => {
	it('uses the fixed hidden root for packaged Desktop', () => {
		expect(
			prepareWithoutFileSystem({
				environment: {},
				homeDirectory: '/Users/person',
				packaged: true,
				platform: 'darwin',
			}),
		).toEqual({ path: '/Users/person/.orchbee', scope: { kind: 'packaged' } });
	});

	it('derives a stable development root from the canonical checkout path', () => {
		const first = prepareWithoutFileSystem(
			{
				environment: { ORCHBEE_DESKTOP_DEVELOPMENT_SOURCE_ROOT: '/source/link' },
				homeDirectory: '/Users/person',
				packaged: false,
				platform: 'darwin',
			},
			'/source/orchbee-worktrees/feature',
		);
		const second = prepareWithoutFileSystem(
			{
				environment: { ORCHBEE_DESKTOP_DEVELOPMENT_SOURCE_ROOT: '/source/original' },
				homeDirectory: '/Users/person',
				packaged: false,
				platform: 'darwin',
			},
			'/source/orchbee-worktrees/feature',
		);
		expect(first).toEqual(second);
		expect(first.path).toMatch(/^\/Users\/person\/\.orchbee-dev-[0-9a-f]{16}$/u);
	});

	it('isolates different development checkouts', () => {
		const resolve = (sourceRoot: string) =>
			prepareWithoutFileSystem(
				{
					environment: { ORCHBEE_DESKTOP_DEVELOPMENT_SOURCE_ROOT: sourceRoot },
					homeDirectory: '/Users/person',
					packaged: false,
					platform: 'darwin',
				},
				sourceRoot,
			);
		expect(resolve('/source/one').path).not.toBe(resolve('/source/two').path);
	});

	it('requires an explicit development checkout root', () => {
		expect(() =>
			prepareWithoutFileSystem({
				environment: {},
				homeDirectory: '/Users/person',
				packaged: false,
				platform: 'darwin',
			}),
		).toThrow('canonical source checkout root');
	});

	it('creates and reuses an agreeing development scope marker', () => {
		const home = temporaryDirectory();
		const input = {
			environment: { ORCHBEE_DESKTOP_DEVELOPMENT_SOURCE_ROOT: home },
			homeDirectory: home,
			packaged: false,
			platform: process.platform,
		} as const;
		const first = prepareDesktopDataRoot(input);
		const second = prepareDesktopDataRoot(input);
		expect(second).toEqual(first);
		expect(
			JSON.parse(readFileSync(path.join(first.path, 'development-scope-v1.json'), 'utf8')),
		).toMatchObject({ kind: 'development', sourceRoot: realpathSync.native(home) });
	});

	it('rejects a development marker for another checkout', () => {
		const home = temporaryDirectory();
		const dependencies: DataRootDependencies = {
			chmod: vi.fn<DataRootDependencies['chmod']>(),
			mkdir: vi.fn<DataRootDependencies['mkdir']>(),
			readFile: vi.fn<DataRootDependencies['readFile']>(() =>
				JSON.stringify({
					digest: '0'.repeat(16),
					digestVersion: 1,
					kind: 'development',
					sourceRoot: '/another/checkout',
				}),
			),
			realpath: vi.fn<DataRootDependencies['realpath']>(() => home),
			writeFile: vi.fn<DataRootDependencies['writeFile']>(() => {
				const error = new Error('exists');
				Object.assign(error, { code: 'EEXIST' });
				throw error;
			}),
		};
		expect(() =>
			prepareDesktopDataRoot(
				{
					environment: { ORCHBEE_DESKTOP_DEVELOPMENT_SOURCE_ROOT: home },
					homeDirectory: home,
					packaged: false,
					platform: process.platform,
				},
				dependencies,
			),
		).toThrow('belongs to another source checkout');
	});
});

function prepareWithoutFileSystem(
	input: Parameters<typeof prepareDesktopDataRoot>[0],
	canonicalSourceRoot = input.homeDirectory,
): ReturnType<typeof prepareDesktopDataRoot> {
	return prepareDesktopDataRoot(input, {
		chmod: () => undefined,
		mkdir: () => undefined,
		readFile: () => {
			throw new Error('Unexpected marker read.');
		},
		realpath: () => canonicalSourceRoot,
		writeFile: () => undefined,
	});
}

function temporaryDirectory(): string {
	const directory = mkdtempSync(path.join(tmpdir(), 'orchbee-data-root-'));
	temporaryDirectories.push(directory);
	return directory;
}
