import { describe, expect, it } from 'vitest';

import {
	locateWindowDesktopRoot,
	parseWindowPointer,
	readFallbackFromVersion,
	resolveWindowUpdateOffer,
} from '../window-update-offer';

const ROOT = 'C:\\Users\\a\\AppData\\Local\\Programs\\Orchbee\\desktop';
const ENTRY = { manifest: 'a'.repeat(64), product: '0.3.0' };

function locate(executablePath: string, overrides: { development?: boolean } = {}) {
	return locateWindowDesktopRoot({
		development: overrides.development ?? false,
		executablePath,
		ownVersion: '0.2.0',
		platform: 'win32',
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
});

describe('readFallbackFromVersion', () => {
	it('reads the version a fallback start came back from, and nothing else', () => {
		expect(readFallbackFromVersion(['orchbee-desktop.exe', '--fallback-from-v1=0.3.0'])).toBe(
			'0.3.0',
		);
		expect(readFallbackFromVersion(['orchbee-desktop.exe'])).toBeNull();
		expect(readFallbackFromVersion(['--fallback-from-v1=not-a-version'])).toBeNull();
		expect(readFallbackFromVersion(['--fallback-from-v1=0.3.0+build'])).toBeNull();
	});
});

describe('parseWindowPointer', () => {
	it('names the active version of a format-1 pointer', () => {
		expect(parseWindowPointer(JSON.stringify({ active: ENTRY, version: 1 }))).toBe('0.3.0');
		expect(
			parseWindowPointer(
				JSON.stringify({
					active: ENTRY,
					extensions: { later: true },
					previous: { ...ENTRY, product: '0.2.0' },
					version: 1,
				}),
			),
		).toBe('0.3.0');
	});

	it('reads anything else as no active version', () => {
		for (const text of [
			'',
			'{',
			JSON.stringify({ active: ENTRY, version: 2 }),
			JSON.stringify({ active: ENTRY, other: 1, version: 1 }),
			JSON.stringify({ active: { ...ENTRY, manifest: 'A'.repeat(64) }, version: 1 }),
			JSON.stringify({ active: { ...ENTRY, product: '01.0.0' }, version: 1 }),
			JSON.stringify({ active: ENTRY, extensions: [], version: 1 }),
		]) {
			expect(parseWindowPointer(text)).toBeNull();
		}
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
