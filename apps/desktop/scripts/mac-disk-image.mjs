// Builds the macOS disk image with a fixed Finder layout, without a Finder session: the image is
// created writable, mounted without Finder, given a background picture and a `.DS_Store` written
// here, then converted to the compressed read-only release format. The layout is that of dmgbuild
// and appdmg (a background alias in the icon view options, one icon location per item); the writer
// is our own so the release needs neither Python nor a native npm module.
import { execFileSync } from 'node:child_process';
import { mkdir, rm, stat, symlink, writeFile } from 'node:fs/promises';
import path from 'node:path';

// Cross-runtime contract: resources/brand/png/dmg/ draws its arrow for this window, these icon
// centres and this icon size, centred between the two icons' visible art (scripts/brand-assets.mjs);
// change both together.
const WINDOW = { x: 200, y: 120, width: 660, height: 400 };
const ICON_SIZE = 128;
const TEXT_SIZE = 12;
const APPLICATION_CENTRE = { x: 180, y: 180 };
const APPLICATIONS_LINK_CENTRE = { x: 480, y: 180 };
// The picture's name at the volume root, hidden from Finder by its dot, as dmgbuild names it.
const BACKGROUND_FILE_NAME = '.background.tiff';

/**
 * Writes `image`, a compressed read-only HFS+ disk image named `volumeName` that holds `application`
 * on the left and a link to `/Applications` on the right, over the background joined from
 * `background.standard` and `background.retina` (twice its size). `workspace` is an empty folder
 * the caller owns and removes.
 */
export async function writeMacDiskImage({ application, volumeName, image, workspace, background }) {
	const volume = path.join(workspace, 'volume');
	await mkdir(volume);
	// ditto keeps the bundle's links, modes and signature.
	execFileSync('/usr/bin/ditto', [application, path.join(volume, path.basename(application))], {
		stdio: 'inherit',
	});
	await symlink('/Applications', path.join(volume, 'Applications'));
	// One TIFF holding both resolutions, so Finder draws the 2x picture on a Retina display.
	execFileSync(
		'/usr/bin/tiffutil',
		[
			'-cathidpicheck',
			background.standard,
			background.retina,
			'-out',
			path.join(volume, BACKGROUND_FILE_NAME),
		],
		{ stdio: 'inherit' },
	);
	const writable = path.join(workspace, 'writable.dmg');
	// GitHub's macOS runners now and then answer `hdiutil create` with "Resource busy"; a later
	// attempt succeeds. The image keeps free space beyond the folder, which the `.DS_Store` needs.
	const megabytes = String(folderMegabytes(volume));
	await retry(async () => {
		await rm(writable, { force: true });
		hdiutil([
			'create',
			'-volname',
			volumeName,
			'-srcfolder',
			volume,
			'-fs',
			'HFS+',
			'-format',
			'UDRW',
			'-megabytes',
			megabytes,
			writable,
		]);
	});
	const mount = path.join(workspace, 'mount');
	await mkdir(mount);
	// Mounted without Finder: hidden from the sidebar and the desktop, with no window.
	const attached = execFileSync(
		'/usr/bin/hdiutil',
		[
			'attach',
			writable,
			'-readwrite',
			'-nobrowse',
			'-noautoopen',
			'-noverify',
			'-plist',
			'-mountpoint',
			mount,
		],
		{ encoding: 'utf8' },
	);
	// The whole disk, whose ejection also unmounts its volume.
	const device = /<string>(\/dev\/disk\d+)<\/string>/u.exec(attached)?.[1];
	let failure;
	try {
		if (device === undefined) throw new Error('hdiutil attach named no disk.');
		await writeLayout(mount, volumeName, path.basename(application));
		await rm(path.join(mount, '.Trashes'), { force: true, recursive: true });
	} catch (error) {
		failure = error;
	}
	execFileSync('/bin/sync');
	await detach(device ?? mount);
	if (failure !== undefined) throw failure;
	hdiutil(['convert', writable, '-format', 'UDZO', '-ov', '-o', image]);
	hdiutil(['verify', image]);
}

function hdiutil(arguments_) {
	execFileSync('/usr/bin/hdiutil', arguments_, { stdio: 'inherit' });
}

/** Runs `attempt` up to three times, five seconds apart. */
async function retry(attempt) {
	for (let count = 1; ; count += 1) {
		try {
			return await attempt();
		} catch (error) {
			if (count === 3) throw error;
			await new Promise((resolve) => setTimeout(resolve, 5000));
		}
	}
}

/**
 * Detaches the writable image. A just-written volume can be busy for a moment, and GitHub's runners
 * have answered "Resource busy" after unmounting the volume but before ejecting the disk; the disk
 * is gone once `hdiutil info` no longer lists it, which ends the attempts as done. The last two
 * attempts force it.
 */
async function detach(device) {
	const attempts = 8;
	for (let count = 1; ; count += 1) {
		const listed = execFileSync('/usr/bin/hdiutil', ['info'], { encoding: 'utf8' });
		if (!new RegExp(`${device}(?![0-9])`, 'u').test(listed)) return;
		try {
			hdiutil(['detach', device, ...(count > attempts - 2 ? ['-force'] : [])]);
			return;
		} catch (error) {
			if (count === attempts) throw error;
			await new Promise((resolve) => setTimeout(resolve, 1000 * 1.5 ** count));
		}
	}
}

/** The image size for `folder`: its bytes with a fifth and 16 MB to spare, in whole megabytes. */
function folderMegabytes(folder) {
	const kilobytes = Number(
		execFileSync('/usr/bin/du', ['-sk', folder], { encoding: 'utf8' }).split(/\s/u)[0],
	);
	return Math.ceil((kilobytes * 1.2) / 1024) + 16;
}

/** Writes the mounted volume's `.DS_Store`: the window, the icon view and both icon locations. */
async function writeLayout(mount, volumeName, applicationFileName) {
	const alias = await backgroundAlias(mount, volumeName);
	const windowSettings = binaryPropertyList({
		ContainerShowSidebar: false,
		PreviewPaneVisibility: false,
		ShowPathbar: false,
		ShowSidebar: false,
		ShowStatusBar: false,
		ShowTabView: false,
		ShowToolbar: false,
		SidebarWidth: 0,
		WindowBounds: `{{${WINDOW.x}, ${WINDOW.y}}, {${WINDOW.width}, ${WINDOW.height}}}`,
	});
	const iconViewSettings = binaryPropertyList({
		arrangeBy: 'none',
		backgroundColorBlue: { real: 1 },
		backgroundColorGreen: { real: 1 },
		backgroundColorRed: { real: 1 },
		backgroundImageAlias: alias,
		// 2: a picture; 1 would be a color, 0 the default window.
		backgroundType: 2,
		gridOffsetX: { real: 0 },
		gridOffsetY: { real: 0 },
		gridSpacing: { real: 100 },
		iconSize: { real: ICON_SIZE },
		labelOnBottom: true,
		scrollPositionX: { real: 0 },
		scrollPositionY: { real: 0 },
		showIconPreview: false,
		showItemInfo: false,
		textSize: { real: TEXT_SIZE },
		viewOptionsVersion: 1,
	});
	const records = [
		dsRecord('.', 'bwsp', 'blob', windowSettings),
		dsRecord('.', 'icvl', 'type', Buffer.from('icnv', 'ascii')),
		dsRecord('.', 'icvp', 'blob', iconViewSettings),
		dsRecord('.', 'vSrn', 'long', 1),
		dsRecord(applicationFileName, 'Iloc', 'blob', iconLocation(APPLICATION_CENTRE)),
		dsRecord('Applications', 'Iloc', 'blob', iconLocation(APPLICATIONS_LINK_CENTRE)),
	];
	await writeFile(path.join(mount, '.DS_Store'), dsStore(records));
}

// --- Alias record ----------------------------------------------------------------------------

const HFS_EPOCH_OFFSET_SECONDS = 2_082_844_800;

/**
 * A version 2 alias record to the background picture, as Finder stores `backgroundImageAlias`:
 * the volume's name and creation date with the file's catalog IDs locate it after the image is
 * mounted anywhere; the paths are the fallbacks. Its mount point is `/Volumes/<volume>`, where a
 * double click mounts it, rather than the build's mount point.
 */
async function backgroundAlias(mount, volumeName) {
	const picture = await stat(path.join(mount, BACKGROUND_FILE_NAME), { bigint: true });
	const root = await stat(mount, { bigint: true });
	const volumeDate = hfsSeconds(root.birthtimeMs);
	const fileDate = hfsSeconds(picture.birthtimeMs);
	const fixed = Buffer.alloc(150);
	fixed.writeUInt16BE(2, 6);
	// Kind 0 is a file.
	fixed.writeUInt16BE(0, 8);
	pascalString(volumeName, 27).copy(fixed, 10);
	fixed.writeUInt32BE(volumeDate, 38);
	fixed.write('H+', 42, 'ascii');
	// Disk type 0, a fixed disk, as mac_alias writes for an image.
	fixed.writeUInt16BE(0, 44);
	fixed.writeUInt32BE(Number(root.ino), 46);
	pascalString(BACKGROUND_FILE_NAME, 63).copy(fixed, 50);
	fixed.writeUInt32BE(Number(picture.ino), 114);
	fixed.writeUInt32BE(fileDate, 118);
	// Creator and type codes stay zero; no levels separate the alias from its target.
	fixed.writeInt16BE(-1, 130);
	fixed.writeInt16BE(-1, 132);
	const tags = [
		aliasTag(0, Buffer.from(macRoman(volumeName), 'latin1')),
		aliasTag(16, highResolutionDate(volumeDate)),
		aliasTag(17, highResolutionDate(fileDate)),
		aliasTag(2, Buffer.from(`${macRoman(volumeName)}:${BACKGROUND_FILE_NAME}`, 'latin1')),
		aliasTag(14, unicodeName(BACKGROUND_FILE_NAME)),
		aliasTag(15, unicodeName(volumeName)),
		aliasTag(18, Buffer.from(`/${BACKGROUND_FILE_NAME}`, 'utf8')),
		aliasTag(19, Buffer.from(`/Volumes/${volumeName}`, 'utf8')),
		Buffer.from([0xff, 0xff, 0, 0]),
	];
	const alias = Buffer.concat([fixed, ...tags]);
	alias.writeUInt16BE(alias.length, 4);
	return alias;
}

function hfsSeconds(milliseconds) {
	return Number(milliseconds / 1000n) + HFS_EPOCH_OFFSET_SECONDS;
}

function highResolutionDate(seconds) {
	const value = Buffer.alloc(8);
	value.writeBigUInt64BE(BigInt(seconds) * 65_536n);
	return value;
}

/** The volume and file names the alias carries are ASCII; a name outside it is refused. */
function macRoman(name) {
	if (!/^[\x20-\x7e]+$/u.test(name)) throw new Error(`The disk image name ${name} is not ASCII.`);
	return name.replaceAll(':', '/');
}

function pascalString(text, capacity) {
	const bytes = Buffer.from(macRoman(text), 'latin1');
	if (bytes.length > capacity) throw new Error(`${text} is longer than ${capacity} bytes.`);
	return Buffer.concat([Buffer.from([bytes.length]), bytes]);
}

function unicodeName(name) {
	const characters = utf16(name.replaceAll(':', '/'));
	const length = Buffer.alloc(2);
	length.writeUInt16BE(characters.length / 2);
	return Buffer.concat([length, characters]);
}

/** One tagged field: tag, length, data padded to an even length. */
function aliasTag(tag, data) {
	const head = Buffer.alloc(4);
	head.writeInt16BE(tag, 0);
	head.writeUInt16BE(data.length, 2);
	return Buffer.concat([head, data, Buffer.alloc(data.length % 2)]);
}

// --- Property lists --------------------------------------------------------------------------

/**
 * A binary property list of a flat dictionary, converted by plutil from XML: a boolean, a whole
 * number as integer, `{ real }` as real, a string, or a Buffer as data.
 */
function binaryPropertyList(dictionary) {
	const value = (entry) => {
		if (typeof entry === 'boolean') return entry ? '<true/>' : '<false/>';
		if (typeof entry === 'number') return `<integer>${entry}</integer>`;
		if (typeof entry === 'string') return `<string>${escapeXml(entry)}</string>`;
		if (Buffer.isBuffer(entry)) return `<data>${entry.toString('base64')}</data>`;
		return `<real>${entry.real}</real>`;
	};
	const body = Object.entries(dictionary)
		.map(([key, entry]) => `<key>${escapeXml(key)}</key>${value(entry)}`)
		.join('');
	const xml = `<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0"><dict>${body}</dict></plist>\n`;
	return execFileSync('/usr/bin/plutil', ['-convert', 'binary1', '-o', '-', '-'], { input: xml });
}

function escapeXml(text) {
	return text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
}

// --- .DS_Store -------------------------------------------------------------------------------

function utf16(text) {
	const little = Buffer.from(text, 'utf16le');
	const big = Buffer.alloc(little.length);
	for (let index = 0; index < little.length; index += 2) {
		big[index] = little[index + 1];
		big[index + 1] = little[index];
	}
	return big;
}

/** The location Finder places an item's icon centre at, in window points. */
function iconLocation({ x, y }) {
	const location = Buffer.alloc(16);
	location.writeUInt32BE(x, 0);
	location.writeUInt32BE(y, 4);
	location.writeUInt32BE(0xffff_ffff, 8);
	location.writeUInt32BE(0xffff_0000, 12);
	return location;
}

/** One `.DS_Store` record: the item's name, a four-letter field, its type and its value. */
function dsRecord(name, field, type, data) {
	const encodedName = utf16(name);
	const head = Buffer.alloc(4);
	head.writeUInt32BE(encodedName.length / 2);
	const codes = Buffer.from(`${field}${type}`, 'ascii');
	let value;
	if (type === 'long') {
		value = Buffer.alloc(4);
		value.writeUInt32BE(data);
	} else if (type === 'type') {
		value = data;
	} else {
		value = Buffer.concat([Buffer.alloc(4), data]);
		value.writeUInt32BE(data.length);
	}
	return { name, field, bytes: Buffer.concat([head, encodedName, codes, value]) };
}

/**
 * The `.DS_Store` file: a buddy-allocated store holding one B-tree whose single leaf carries every
 * record, sorted by name without regard to case and then by field, as Finder sorts them. Offsets
 * count from byte 4; the header occupies the first 32 bytes, then the tree's root record, the
 * allocator's own block and the leaf, each aligned to its power-of-two size, with every other
 * buddy on the free lists.
 */
function dsStore(records) {
	const sorted = records.toSorted((left, right) => {
		const leftName = left.name.toLowerCase();
		const rightName = right.name.toLowerCase();
		if (leftName !== rightName) return leftName < rightName ? -1 : 1;
		if (left.field === right.field) return 0;
		return left.field < right.field ? -1 : 1;
	});
	const pageSize = 0x1000;
	const leaf = Buffer.concat([u32(0), u32(sorted.length), ...sorted.map((record) => record.bytes)]);
	if (leaf.length > pageSize) throw new Error('The disk image layout outgrew one .DS_Store page.');
	// Block 0: the allocator, 2048 bytes at 2048; 1: the tree, 32 at 32; 2: the leaf, 4096 at 4096.
	const blocks = [
		[2048, 11],
		[32, 5],
		[4096, 12],
	];
	const tree = Buffer.concat([u32(2), u32(0), u32(sorted.length), u32(1), u32(pageSize)]);
	const addresses = Buffer.alloc(256 * 4);
	blocks.forEach(([offset, log2], index) => addresses.writeUInt32BE(offset | log2, index * 4));
	const free = new Map([
		[6, 64],
		[7, 128],
		[8, 256],
		[9, 512],
		[10, 1024],
	]);
	for (let log2 = 13; log2 <= 30; log2 += 1) free.set(log2, 2 ** log2);
	const freeLists = Array.from({ length: 32 }, (_, log2) =>
		free.has(log2) ? Buffer.concat([u32(1), u32(free.get(log2))]) : u32(0),
	);
	const allocator = Buffer.concat([
		u32(blocks.length),
		u32(0),
		addresses,
		u32(1),
		Buffer.from([4]),
		Buffer.from('DSDB', 'ascii'),
		u32(1),
		...freeLists,
	]);
	const file = Buffer.alloc(4 + 8192);
	file.writeUInt32BE(1, 0);
	file.write('Bud1', 4, 'ascii');
	file.writeUInt32BE(2048, 8);
	file.writeUInt32BE(2048, 12);
	file.writeUInt32BE(2048, 16);
	// Sixteen bytes of no known meaning that Finder's own files carry here, as the ds_store library
	// writes them.
	Buffer.from('0000100c000000870000200b00000000', 'hex').copy(file, 20);
	allocator.copy(file, 4 + 2048);
	tree.copy(file, 4 + 32);
	leaf.copy(file, 4 + 4096);
	return file;
}

function u32(value) {
	const bytes = Buffer.alloc(4);
	bytes.writeUInt32BE(value);
	return bytes;
}
