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
	it('uses separate fixed hidden roots for local and release Desktop', () => {
		expect(
			prepareWithoutFileSystem({
				distribution: resolveDesktopDistributionProfile('release'),
				homeDirectory: '/Users/person',
				platform: 'darwin',
			}),
		).toEqual({ path: '/Users/person/.orchbee', scope: { kind: 'release' } });
		expect(
			prepareWithoutFileSystem({
				distribution: resolveDesktopDistributionProfile('local', undefined, {
					artifactStem: 'ob-test-0123456789abcdef',
				}),
				homeDirectory: '/Users/person',
				platform: 'darwin',
			}),
		).toEqual({ path: '/Users/person/.orchbee-local', scope: { kind: 'local' } });
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
		expect(first.path).toMatch(/^\/Users\/person\/\.orchbee-dev-[0-9a-f]{16}$/u);
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
			distribution: developmentDistribution(realpathSync.native(home), process.platform),
			homeDirectory: home,
			platform: process.platform,
		} as const;
		const first = prepareDesktopDataRoot(input);
		const second = prepareDesktopDataRoot(input);
		expect(second).toEqual(first);
		expect(
			JSON.parse(readFileSync(path.join(first.path, 'development-scope-v1.json'), 'utf8')),
		).toMatchObject(input.distribution.dataScope);
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
					distribution: developmentDistribution(home, process.platform),
					homeDirectory: home,
					platform: process.platform,
				},
				dependencies,
			),
		).toThrow('belongs to another source checkout');
	});
});

function prepareWithoutFileSystem(
	input: Parameters<typeof prepareDesktopDataRoot>[0],
): ReturnType<typeof prepareDesktopDataRoot> {
	return prepareDesktopDataRoot(input, {
		chmod: () => undefined,
		mkdir: () => undefined,
		readFile: () => {
			throw new Error('Unexpected marker read.');
		},
		writeFile: () => undefined,
	});
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
