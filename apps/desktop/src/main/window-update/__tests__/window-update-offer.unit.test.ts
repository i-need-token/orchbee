import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import {
	locateWindowDesktopRoot,
	parseWindowPointer,
	readFallbackFromVersion,
	resolvePosixReleaseStoreRoot,
	resolveWindowUpdateOffer,
} from '../window-update-offer';

const ROOT = 'C:\\Users\\a\\AppData\\Local\\Programs\\Orchbee\\desktop';
const STORE = '/Users/a/.local/share/orchbee';

function locate(executablePath: string, overrides: { development?: boolean } = {}) {
	return locateWindowDesktopRoot({
		development: overrides.development ?? false,
		executablePath,
		ownVersion: '0.2.0',
		platform: 'win32',
		storeRoot: null,
	});
}

function locateMac(executablePath: string, storeRoot: string | null = STORE) {
	return locateWindowDesktopRoot({
		development: false,
		executablePath,
		ownVersion: '0.2.0',
		platform: 'darwin',
		storeRoot,
	});
}

describe('locateWindowDesktopRoot', () => {
	it('finds desktop\\ only for a window in the version folder of its own version', () => {
		expect(locate(`${ROOT}\\versions\\0.2.0\\orchbee-desktop.exe`)).toBe(ROOT);
		expect(locate(`${ROOT.toUpperCase()}\\VERSIONS\\0.2.0\\orchbee-desktop.exe`)).toBe(
			ROOT.toUpperCase(),
		);
	});

	it('never offers from a flat layout, another version folder, a build folder, or a development build', () => {
		expect(locate(`${ROOT}\\orchbee-desktop.exe`)).toBeNull();
		expect(locate(`${ROOT}\\versions\\0.1.0\\orchbee-desktop.exe`)).toBeNull();
		expect(locate('C:\\src\\orchbee\\apps\\desktop\\out\\0.2.0\\orchbee-desktop.exe')).toBeNull();
		expect(locate('C:\\elsewhere\\versions\\0.2.0\\orchbee-desktop.exe')).toBeNull();
		expect(locate('versions\\0.2.0\\orchbee-desktop.exe')).toBeNull();
		expect(
			locate(`${ROOT}\\versions\\0.2.0\\orchbee-desktop.exe`, { development: true }),
		).toBeNull();
	});

	// The macOS launcher replaces the bundle the window runs from, so the rule decides what it renames.
	it('on macOS reads the store for a window in any application bundle outside the store', () => {
		expect(locateMac('/Applications/Orchbee.app/Contents/MacOS/Orchbee')).toBe(`${STORE}/desktop`);
		expect(locateMac('/Users/a/Applications/Orchbee Test.app/Contents/MacOS/Orchbee Test')).toBe(
			`${STORE}/desktop`,
		);
	});

	it('on macOS never offers outside a bundle, without a store root, or from the store’s own copies', () => {
		expect(locateMac('/Applications/Orchbee')).toBeNull();
		expect(locateMac('/Applications/Orchbee.app/Contents/Resources/Orchbee')).toBeNull();
		expect(locateMac('/Applications/Orchbee.app/Contents/MacOS/Orchbee', null)).toBeNull();
		expect(
			locateMac(`${STORE}/desktop/versions/0.2.0/Orchbee.app/Contents/MacOS/Orchbee`),
		).toBeNull();
		expect(
			locateMac(`${STORE.toUpperCase()}/DESKTOP/versions/0.2.0/Orchbee.app/Contents/MacOS/Orchbee`),
		).toBeNull();
	});
});

describe('resolvePosixReleaseStoreRoot', () => {
	it('follows the Client’s rule and refuses what it refuses', () => {
		expect(resolvePosixReleaseStoreRoot({ HOME: '/Users/a' }, 'orchbee')).toBe(STORE);
		expect(resolvePosixReleaseStoreRoot({ HOME: '/Users/a' }, 'ob-test-0123456789abcdef')).toBe(
			'/Users/a/.local/share/ob-test-0123456789abcdef',
		);
		expect(
			resolvePosixReleaseStoreRoot({ HOME: '/Users/a', ORCHBEE_INSTALL_DIR: '/opt/ob' }, 'orchbee'),
		).toBe('/opt/ob');
		for (const configured of ['relative', '/a/../b', '/a/./b', '/a:b', '/a$b', '/a\nb']) {
			expect(
				resolvePosixReleaseStoreRoot({ ORCHBEE_INSTALL_DIR: configured }, 'orchbee'),
			).toBeNull();
		}
		expect(resolvePosixReleaseStoreRoot({}, 'orchbee')).toBeNull();
	});
});

describe('readFallbackFromVersion', () => {
	it('reads the version a fallback start came back from, and nothing else', () => {
		expect(readFallbackFromVersion(['orchbee-desktop.exe', '--fallback-from=0.3.0'])).toBe('0.3.0');
		expect(readFallbackFromVersion(['orchbee-desktop.exe'])).toBeNull();
		expect(readFallbackFromVersion(['--fallback-from=not-a-version'])).toBeNull();
		expect(readFallbackFromVersion(['--fallback-from=0.3.0+build'])).toBeNull();
	});
});

/** The pointer samples every reader of the format shares (`crates/release-records`). */
const POINTER_SAMPLES = z
	.strictObject({
		cases: z.array(
			z.strictObject({
				active: z.string().nullable(),
				held: z.array(z.string()),
				name: z.string(),
				pointer: z.unknown().optional(),
				text: z.string().optional(),
			}),
		),
	})
	.parse(
		JSON.parse(
			readFileSync(
				new URL('../../../../../../crates/release-records/fixtures/pointers.json', import.meta.url),
				'utf8',
			),
		),
	).cases;

describe('parseWindowPointer', () => {
	it.each(POINTER_SAMPLES)('reads "$name" as the release records crate does', (sample) => {
		expect(parseWindowPointer(sample.text ?? JSON.stringify(sample.pointer))).toBe(sample.active);
	});
});

describe('resolveWindowUpdateOffer', () => {
	const offer = (activeVersion: string | null, fallbackFrom: string | null = null) =>
		resolveWindowUpdateOffer({ activeVersion, fallbackFrom, ownVersion: '0.2.0-beta.4' });

	it('offers an active window version of higher precedence than its own', () => {
		expect(offer('0.2.0-beta.5')).toBe('0.2.0-beta.5');
		expect(offer('0.2.0')).toBe('0.2.0');
	});

	it('offers nothing for its own, a lower, or no active version', () => {
		expect(offer('0.2.0-beta.4')).toBeNull();
		expect(offer('0.2.0-beta.3')).toBeNull();
		expect(offer(null)).toBeNull();
	});

	it('offers nothing for the version a fallback start came back from, but a newer one as usual', () => {
		expect(offer('0.2.0', '0.2.0')).toBeNull();
		expect(offer('0.2.1', '0.2.0')).toBe('0.2.1');
	});
});
