import { mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	createDesktopDevelopmentScope,
	resolveDesktopDistributionProfile,
} from '../../../distribution';
import { DesktopDataRootRefusal } from '../data-root-version-record';
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
	});

	it('keeps the newest version current and refuses a higher or unreadable layout untouched', () => {
		const home = temporaryDirectory();
		const input = {
			applicationVersion: '1.2.3',
			distribution: developmentDistribution(realpathSync.native(home), process.platform),
			homeDirectory: home,
			homeOverride: undefined,
			platform: process.platform,
		} as const;
		const root = prepareDesktopDataRoot(input);
		const record = path.join(root.path, 'internal', 'version.json');
		writeFileSync(record, '{"layout":1,"newestVersion":"1.2.3","later":[1]}\n');
		for (const applicationVersion of ['2.0.0-beta.2', '2.0.0', '1.9.0', '2.0.0-beta.10']) {
			prepareDesktopDataRoot({ ...input, applicationVersion });
		}
		expect(JSON.parse(readFileSync(record, 'utf8'))).toEqual({
			later: [1],
			layout: 1,
			newestVersion: '2.0.0',
		});
		for (const text of ['{"layout":2,"newestVersion":"9.0.0"}', '{"layout":"1"}']) {
			writeFileSync(record, text);
			expect(() => prepareDesktopDataRoot(input)).toThrow(DesktopDataRootRefusal);
			expect(readFileSync(record, 'utf8')).toBe(text);
		}
	});

	it('rejects a development marker for another checkout', () => {
		const home = temporaryDirectory();
		const dependencies: DataRootDependencies = {
			chmod: vi.fn<DataRootDependencies['chmod']>(),
			mkdir: vi.fn<DataRootDependencies['mkdir']>(),
			readFile: vi.fn<DataRootDependencies['readFile']>((filePath) => {
				if (filePath.endsWith('version.json')) throw missingFile();
				return JSON.stringify({
					digest: '0'.repeat(16),
					digestVersion: 1,
					kind: 'development',
					sourceRoot: '/another/checkout',
				});
			}),
			rename: vi.fn<DataRootDependencies['rename']>(),
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
			readFile: (filePath) => {
				if (filePath.endsWith('version.json')) throw missingFile();
				throw new Error('Unexpected marker read.');
			},
			rename: () => undefined,
			writeFile: () => undefined,
		},
	);
}

function missingFile(): Error {
	return Object.assign(new Error('missing'), { code: 'ENOENT' });
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
