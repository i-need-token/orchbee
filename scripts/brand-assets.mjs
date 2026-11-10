// Brand asset definitions derived from resources/icon.svg. The SVG outputs are exact text; raster
// and container outputs are rendered from those SVGs and checked by inventory and dimensions.

const INK = '#16140F';
const GLYPH = '#F8F7F4';
const MASTER_OPENING_TAG = /^\s*<svg\s[^>]*\bviewBox="0 0 1024 1024"[^>]*>/u;
const MASTER_TILE_PATH = /<path fill="#16140F" d="[^"]+"\/>/u;
// The macOS application icon grid draws an 824 px body centred on the 1024 px canvas.
const DARWIN_BODY_TRANSFORM = 'translate(100 100) scale(0.8046875)';

const DARWIN_BADGES = {
	dev: `<g data-orchbee-distribution="dev"><circle cx="836" cy="836" r="140" fill="#625bf6" stroke="${INK}" stroke-width="24"/><path d="M812 768 754 836l58 68m48-136 58 68-58 68" fill="none" stroke="${GLYPH}" stroke-linecap="round" stroke-linejoin="round" stroke-width="30"/></g>`,
	test: `<g data-orchbee-distribution="test"><rect x="696" y="696" width="280" height="280" rx="90" fill="#168f5b" stroke="${INK}" stroke-width="24"/><path d="M766 856v-40a70 70 0 0 1 140 0v40m-160 0h180v70H746z" fill="none" stroke="${GLYPH}" stroke-linecap="round" stroke-linejoin="round" stroke-width="30"/></g>`,
	release: '',
};

// The Windows notification-area icon: the mark, with the most urgent Agent state as a hexagon in
// the lower-right corner, in the fixed status colors every theme uses, and outside a release the
// environment's badge in the upper-left corner, a circle for Dev and a square for Test, so neither
// reads as a state. Their ink outline keeps them apart from the hive ring at 16 px.
const TRAY_STATUS_COLORS = {
	none: null,
	failed: '#FF5F57',
	attention: '#FEBC2E',
	done: '#28C840',
};
const TRAY_BADGES = {
	release: '',
	dev: `<circle cx="248" cy="248" r="200" fill="#625bf6" stroke="${INK}" stroke-width="48"/>`,
	test: `<rect x="56" y="56" width="384" height="384" rx="72" fill="#168f5b" stroke="${INK}" stroke-width="48"/>`,
};
// A pointy-top hexagon, the TUI's ⬢, unlike the hive's flat-top cells.
const TRAY_STATUS_HEXAGON = 'M760 520 967.8 640V880L760 1000 552.2 880V640Z';
const TRAY_ICON_SIZES = [16, 20, 24, 32, 40, 48];

const ICON_SIZES = [16, 20, 24, 32, 40, 48, 64, 96, 128, 180, 192, 256, 512, 1024, 2048];
const DARWIN_SIZES = [16, 32, 64, 128, 256, 512, 1024];
const JPEG_SIZES = [256, 512, 1024, 2048];
const ICNS_TYPES = [
	['icp4', 16],
	['icp5', 32],
	['icp6', 64],
	['ic07', 128],
	['ic08', 256],
	['ic09', 512],
	['ic10', 1024],
	['ic11', 32],
	['ic12', 64],
	['ic13', 256],
	['ic14', 512],
];
const FAVICON_SIZES = [16, 32, 48];
const WINDOWS_ICON_SIZES = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256];
const SOCIAL_CARDS = [
	['light', '#EFEDE8', 'icon'],
	['dark', INK, 'square'],
];

const darwinSuffix = (variant) => (variant === 'release' ? '' : `-${variant}`);
const traySuffix = (status) => (status === 'none' ? '' : `-${status}`);
const trayName = (variant, status) => `orchbee${darwinSuffix(variant)}${traySuffix(status)}`;

export function renderBrandSvgs(masterSvg) {
	const master = masterSvg.trim();
	const openingTag = MASTER_OPENING_TAG.exec(master)?.[0];
	const tilePath = MASTER_TILE_PATH.exec(master)?.[0];
	if (openingTag === undefined || tilePath === undefined || !master.endsWith('</svg>')) {
		throw new Error('resources/icon.svg must be one 1024 px SVG whose first path is the ink tile.');
	}
	const artwork = master.slice(openingTag.length, -'</svg>'.length);
	const svgs = new Map([
		['icon', master],
		['square', master.replace(tilePath, `<rect width="1024" height="1024" fill="${INK}"/>`)],
	]);
	for (const [variant, badge] of Object.entries(DARWIN_BADGES)) {
		svgs.set(
			`macos${darwinSuffix(variant)}`,
			`<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024"><g transform="${DARWIN_BODY_TRANSFORM}">${artwork}</g>${badge}</svg>`,
		);
	}
	for (const [variant, badge] of Object.entries(TRAY_BADGES)) {
		for (const [status, color] of Object.entries(TRAY_STATUS_COLORS)) {
			const hexagon =
				color === null
					? ''
					: `<path d="${TRAY_STATUS_HEXAGON}" fill="${color}" stroke="${INK}" stroke-width="48" stroke-linejoin="round"/>`;
			svgs.set(
				`tray-${trayName(variant, status)}`,
				`<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">${artwork}${badge}${hexagon}</svg>`,
			);
		}
	}
	return svgs;
}

// Every tracked file under resources/brand/, keyed by its relative path.
export function brandAssetPlan() {
	const plan = new Map();
	for (const name of ['square', 'macos', 'macos-dev', 'macos-test']) {
		plan.set(`svg/orchbee-icon-${name}.svg`, { kind: 'svg', svg: name });
	}
	for (const size of ICON_SIZES) {
		plan.set(`png/icon/orchbee-icon-${size}.png`, {
			kind: 'png',
			svg: 'icon',
			width: size,
			height: size,
		});
		plan.set(`png/square/orchbee-icon-square-${size}.png`, {
			kind: 'png',
			svg: 'square',
			width: size,
			height: size,
		});
	}
	for (const variant of Object.keys(DARWIN_BADGES)) {
		const name = `macos${darwinSuffix(variant)}`;
		for (const size of DARWIN_SIZES) {
			plan.set(`png/macos/orchbee-icon-${name}-${size}.png`, {
				kind: 'png',
				svg: name,
				width: size,
				height: size,
			});
		}
		plan.set(`icns/orchbee${darwinSuffix(variant)}.icns`, {
			kind: 'icns',
			svg: name,
			entries: ICNS_TYPES,
		});
	}
	for (const size of JPEG_SIZES) {
		plan.set(`jpg/orchbee-icon-${size}-on-white.jpg`, {
			kind: 'jpeg',
			svg: 'icon',
			width: size,
			height: size,
			background: '#FFFFFF',
		});
		plan.set(`jpg/orchbee-icon-square-${size}.jpg`, {
			kind: 'jpeg',
			svg: 'square',
			width: size,
			height: size,
			background: INK,
		});
	}
	plan.set('ico/favicon.ico', { kind: 'ico', svg: 'icon', sizes: FAVICON_SIZES });
	plan.set('ico/orchbee.ico', { kind: 'ico', svg: 'icon', sizes: WINDOWS_ICON_SIZES });
	for (const variant of Object.keys(TRAY_BADGES)) {
		for (const status of Object.keys(TRAY_STATUS_COLORS)) {
			const name = trayName(variant, status);
			plan.set(`ico/tray/${name}.ico`, {
				kind: 'ico',
				svg: `tray-${name}`,
				sizes: TRAY_ICON_SIZES,
			});
		}
	}
	for (const [name, background, svg] of SOCIAL_CARDS) {
		for (const [kind, extension] of [
			['png', 'png'],
			['jpeg', 'jpg'],
		]) {
			plan.set(`social/orchbee-og-1200x630-${name}.${extension}`, {
				kind,
				svg,
				width: 1200,
				height: 630,
				background,
				markSize: 360,
			});
		}
	}
	return plan;
}

export function encodeIco(entries) {
	const header = Buffer.alloc(6 + 16 * entries.length);
	header.writeUInt16LE(1, 2);
	header.writeUInt16LE(entries.length, 4);
	let offset = header.length;
	entries.forEach(([size, png], index) => {
		const entry = 6 + 16 * index;
		header.writeUInt8(size >= 256 ? 0 : size, entry);
		header.writeUInt8(size >= 256 ? 0 : size, entry + 1);
		header.writeUInt16LE(1, entry + 4);
		header.writeUInt16LE(32, entry + 6);
		header.writeUInt32LE(png.length, entry + 8);
		header.writeUInt32LE(offset, entry + 12);
		offset += png.length;
	});
	return Buffer.concat([header, ...entries.map(([, png]) => png)]);
}

export function encodeIcns(entries) {
	const chunks = entries.map(([type, png]) => {
		const head = Buffer.alloc(8);
		head.write(type, 0, 'ascii');
		head.writeUInt32BE(png.length + 8, 4);
		return Buffer.concat([head, png]);
	});
	const head = Buffer.alloc(8);
	head.write('icns', 0, 'ascii');
	head.writeUInt32BE(8 + chunks.reduce((length, chunk) => length + chunk.length, 0), 4);
	return Buffer.concat([head, ...chunks]);
}

function pngSize(buffer, offset = 0) {
	if (buffer.toString('latin1', offset + 1, offset + 4) !== 'PNG') return null;
	return [buffer.readUInt32BE(offset + 16), buffer.readUInt32BE(offset + 20)];
}

function jpegSize(buffer) {
	let offset = 2;
	while (offset + 9 < buffer.length && buffer[offset] === 0xff) {
		const marker = buffer[offset + 1];
		if (marker >= 0xc0 && marker <= 0xc3) {
			return [buffer.readUInt16BE(offset + 7), buffer.readUInt16BE(offset + 5)];
		}
		offset += 2 + buffer.readUInt16BE(offset + 2);
	}
	return null;
}

// Returns a problem description, or null when the file matches its plan entry.
export function inspectBrandAsset(entry, buffer, svgs) {
	const same = (actual, expected) => JSON.stringify(actual) === JSON.stringify(expected);
	switch (entry.kind) {
		case 'svg':
			return buffer.toString('utf8') === `${svgs.get(entry.svg)}\n` ? null : 'stale SVG';
		case 'png':
			return same(pngSize(buffer), [entry.width, entry.height]) ? null : 'wrong PNG size';
		case 'jpeg':
			return same(jpegSize(buffer), [entry.width, entry.height]) ? null : 'wrong JPEG size';
		case 'ico': {
			const sizes = Array.from({ length: buffer.readUInt16LE(4) }, (_, index) => {
				const offset = buffer.readUInt32LE(6 + 16 * index + 12);
				return pngSize(buffer, offset)?.[0];
			});
			return same(sizes, entry.sizes) ? null : 'wrong ICO entries';
		}
		case 'icns': {
			const found = [];
			for (let offset = 8; offset + 8 <= buffer.length; offset += buffer.readUInt32BE(offset + 4)) {
				found.push([
					buffer.toString('ascii', offset, offset + 4),
					pngSize(buffer, offset + 8)?.[0],
				]);
			}
			return same(found, entry.entries) ? null : 'wrong ICNS entries';
		}
		default:
			return 'unknown asset kind';
	}
}
