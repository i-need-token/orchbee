import { mkdtempSync, readFileSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	createDesktopDevelopmentScope,
	resolveDesktopDistributionProfile,
} from '../../../distribution';
import { prepareDesktopDataRoot } from '../prepareDesktopDataRoot';

type DataRootDependencies = NonNullable<Parameters<typeof prepareDesktopDataRoot>[1]>;

const temporaryDirectories: string[] = [];

afterEach(() => {
	for (const directory of temporaryDirectories.splice(0)) {
		rmSync(directory, { force: true, recursive: true });
	}
});

describe('prepareDesktopDataRoot', () => {
	it('uses the fixed hidden root for release Desktop and keeps its own state internal', () => {
		const root = prepareWithoutFileSystem({
			distribution: resolveDesktopDistributionProfile('release'),
			homeDirectory: '/Users/person',
			platform: 'darwin',
		});
		expect(root.path).toBe('/Users/person/.orchbee');
		expect(root.scope).toEqual({ kind: 'release' });
		for (const location of [root.electronDirectory, root.windowStateFile]) {
			expect(location.startsWith('/Users/person/.orchbee/internal/')).toBe(true);
		}
	});

	it('takes the root from ORCHBEE_HOME only when it names an absolute folder', () => {
		const prepare = (homeOverride: string) =>
			prepareWithoutFileSystem({
				distribution: developmentDistribution('/source/orchbee'),
				homeDirectory: '/Users/person',
				homeOverride,
				platform: 'darwin',
			});
		expect(prepare('/Volumes/data/orchbee').path).toBe('/Volumes/data/orchbee');
		expect(() => prepare('relative/orchbee')).toThrow('ORCHBEE_HOME');
	});

	it('derives a stable development root from the canonical checkout path', () => {
		const first = prepareWithoutFileSystem({
			distribution: developmentDistribution('/source/orchbee-worktrees/feature'),
			homeDirectory: '/Users/person',
			platform: 'darwin',
		});
		const second = prepareWithoutFileSystem({
			distribution: developmentDistribution('/source/orchbee-worktrees/feature'),
			homeDirectory: '/Users/person',
			platform: 'darwin',
		});
		expect(first).toEqual(second);
	});

	it('isolates different development checkouts', () => {
		const resolve = (sourceRoot: string) =>
			prepareWithoutFileSystem({
				distribution: developmentDistribution(sourceRoot),
				homeDirectory: '/Users/person',
				platform: 'darwin',
			});
		expect(resolve('/source/one').path).not.toBe(resolve('/source/two').path);
	});

	it('creates and reuses an agreeing development scope marker', () => {
		const home = temporaryDirectory();
		const input = {
			applicationVersion: '1.2.3',
			distribution: developmentDistribution(realpathSync.native(home), process.platform),
			homeDirectory: home,
			homeOverride: undefined,
			platform: process.platform,
		} as const;
		const first = prepareDesktopDataRoot(input);
		const second = prepareDesktopDataRoot({ ...input, applicationVersion: '2.0.0' });
		expect(second).toEqual(first);
		const internal = path.join(first.path, 'internal');
		expect(
			JSON.parse(readFileSync(path.join(internal, 'development-checkout.json'), 'utf8')),
		).toMatchObject(input.distribution.dataScope);
		// The root records its layout once; a later start leaves the record as it is.
		expect(JSON.parse(readFileSync(path.join(internal, 'version.json'), 'utf8'))).toEqual({
			layout: 1,
			newestVersion: '1.2.3',
		});
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
			writeFile: vi.fn<DataRootDependencies['writeFile']>(() => {
				const error = new Error('exists');
				Object.assign(error, { code: 'EEXIST' });
				throw error;
			}),
		};
		expect(() =>
			prepareDesktopDataRoot(
				{
					applicationVersion: '1.2.3',
					distribution: developmentDistribution(home, process.platform),
					homeDirectory: home,
					homeOverride: undefined,
					platform: process.platform,
				},
				dependencies,
			),
		).toThrow('belongs to another source checkout');
	});
});

function prepareWithoutFileSystem(
	input: Omit<
		Parameters<typeof prepareDesktopDataRoot>[0],
		'applicationVersion' | 'homeOverride'
	> & {
		readonly homeOverride?: string;
	},
): ReturnType<typeof prepareDesktopDataRoot> {
	return prepareDesktopDataRoot(
		{ applicationVersion: '1.2.3', homeOverride: undefined, ...input },
		{
			chmod: () => undefined,
			mkdir: () => undefined,
			readFile: () => {
				throw new Error('Unexpected marker read.');
			},
			writeFile: () => undefined,
		},
	);
}

function developmentDistribution(sourceRoot: string, platform: NodeJS.Platform = 'darwin') {
	return resolveDesktopDistributionProfile(
		'dev',
		createDesktopDevelopmentScope(sourceRoot, platform),
	);
}

function temporaryDirectory(): string {
	const directory = mkdtempSync(path.join(tmpdir(), 'orchbee-data-root-'));
	temporaryDirectories.push(directory);
	return directory;
}
