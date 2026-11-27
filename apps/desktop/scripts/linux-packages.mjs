// The Linux window's menu files and installers. Every Linux window version carries its menu entry
// template and icons as `resources/linux/<stem>.desktop` and `resources/linux/icons/<n>x<n>.png`;
// the window store copies them out with the entry's path in `Exec=`
// (apps/orchbee/src/cli/update/menu_entry.rs), and the installers install their own copies.
//
// The AppImage is the packaged application with an `AppRun` beside it, in a SquashFS image after
// the AppImage runtime. `ORCHBEE_APPIMAGE_RUNTIME` names that runtime by an absolute path, built
// from a pinned AppImage/type2-runtime commit as the release workflow builds it, and `mksquashfs`
// (squashfs-tools) makes the image. The Debian package installs the application under
// `/opt/<stem>/` with its menu entry, its icons and an AppArmor profile that lets it create the user
// namespaces Chromium's sandbox needs where the system restricts them, as Ubuntu 23.10 and later
// do; `dpkg-deb` builds it. Where the system restricts them and no profile covers the program, the
// AppImage, like the store window, starts without the sandbox. Node.js built-ins only.
import { execFileSync } from 'node:child_process';
import {
	chmod,
	copyFile,
	cp,
	mkdir,
	open,
	readFile,
	readdir,
	stat,
	writeFile,
} from 'node:fs/promises';
import path from 'node:path';

/** The hicolor sizes the brand's icon set provides and the menu entry installs. */
const ICON_SIZES = [16, 24, 32, 48, 64, 128, 256, 512];
/** The `.deb` maintainer the owner gave for public use. */
const MAINTAINER = 'Orchbee <ineedtoken001@gmail.com>';
const HOMEPAGE = 'https://orchbee.com';
// The libraries Electron loads beyond the C library, by their Debian names; Ubuntu 24.04 renamed
// some for its 64-bit time transition.
const DEPENDS = [
	'libgtk-3-0 | libgtk-3-0t64',
	'libnss3',
	'libgbm1',
	'libasound2 | libasound2t64',
	'libxss1',
	'libxtst6',
	'libatspi2.0-0 | libatspi2.0-0t64',
	'xdg-utils',
];
/** The Debian architecture and the ELF machine of each Linux release platform. */
const ARCHITECTURES = {
	'linux-x86_64': { debian: 'amd64', machine: 62 },
	'linux-aarch64': { debian: 'arm64', machine: 183 },
};

/**
 * The menu entry of the program `exec`: the window is named `applicationName`, its icon and the
 * entry's file are named after the environment stem, and on X11 its windows group under the entry by
 * the class Electron gives them, the application's name.
 */
function menuEntry({ applicationName, exec, stem }) {
	if (/\p{Cc}/u.test(applicationName))
		throw new Error('The application name holds a control character');
	return [
		'[Desktop Entry]',
		'Type=Application',
		`Name=${applicationName}`,
		`Exec=${exec}`,
		`Icon=${stem}`,
		'Terminal=false',
		'Categories=System;TerminalEmulator;Development;',
		`StartupWMClass=${applicationName}`,
		'',
	].join('\n');
}

/**
 * Writes the menu files every Linux window version carries into `directory`: `<stem>.desktop`, whose
 * `Exec=` the store and the installers replace, and `icons/<n>x<n>.png` from the brand's icon set.
 */
export async function writeLinuxMenuFiles({ applicationName, directory, repositoryRoot, stem }) {
	await mkdir(path.join(directory, 'icons'), { recursive: true });
	await writeFile(
		path.join(directory, `${stem}.desktop`),
		menuEntry({ applicationName, exec: `${stem}-desktop`, stem }),
	);
	await Promise.all(
		ICON_SIZES.map((size) =>
			copyFile(
				path.join(repositoryRoot, 'resources', 'brand', 'png', 'icon', `orchbee-icon-${size}.png`),
				path.join(directory, 'icons', `${size}x${size}.png`),
			),
		),
	);
}

/**
 * Whether the system restricts unprivileged user namespaces, as `sh` tests it: Ubuntu's AppArmor
 * switch, Debian's older switch, or no user namespaces at all. The store window's launcher reads the
 * same three files (apps/orchbee-desktop-launcher/src/linux.rs).
 */
const RESTRICTED = [
	'[ "$(cat /proc/sys/kernel/apparmor_restrict_unprivileged_userns 2>/dev/null)" = 1 ]',
	'[ "$(cat /proc/sys/kernel/unprivileged_userns_clone 2>/dev/null)" = 0 ]',
	'[ "$(cat /proc/sys/user/max_user_namespaces 2>/dev/null)" = 0 ]',
].join(' || ');

function appRun(stem) {
	return [
		'#!/bin/sh',
		'# Starts the window from the mounted image. Where the system restricts unprivileged user',
		"# namespaces, Chromium's sandbox cannot start without a profile this image cannot install, so",
		'# the window starts without it there.',
		'here=$(dirname "$(readlink -f "$0")")',
		`if ${RESTRICTED}; then`,
		`\texec "$here/${stem}-desktop" --no-sandbox "$@"`,
		'fi',
		`exec "$here/${stem}-desktop" "$@"`,
		'',
	].join('\n');
}

function appArmorProfile(stem) {
	return [
		'# Lets the Orchbee window create the user namespaces its Chromium sandbox needs.',
		'abi <abi/4.0>,',
		'include <tunables/global>',
		'',
		`profile ${stem} /opt/${stem}/${stem}-desktop flags=(unconfined) {`,
		'  userns,',
		'',
		`  include if exists <local/${stem}>`,
		'}',
		'',
	].join('\n');
}

/**
 * The maintainer scripts: the profile is installed and loaded only where this AppArmor reads it, so
 * a system without AppArmor 4 installs the package without one, and removed with the package. Where
 * the kernel gives unprivileged users no user namespaces at all, no profile helps, and Chromium's
 * sandbox runs through its setuid helper instead, as Chrome's own package sets it.
 */
function postinst(stem) {
	return [
		'#!/bin/sh',
		'set -e',
		`source=/usr/share/${stem}/apparmor-profile`,
		`target=/etc/apparmor.d/${stem}`,
		'if [ "$1" = configure ] && command -v apparmor_parser >/dev/null 2>&1 &&',
		'\tapparmor_parser --skip-kernel-load "$source" >/dev/null 2>&1; then',
		'\tcp -f "$source" "$target"',
		'\tif [ -d /sys/kernel/security/apparmor ] && ! { [ -x /usr/bin/ischroot ] && /usr/bin/ischroot; }; then',
		'\t\tapparmor_parser --replace --write-cache --skip-read-cache "$target"',
		'\tfi',
		'fi',
		`helper=/opt/${stem}/chrome-sandbox`,
		'if [ "$1" = configure ] && [ -f "$helper" ]; then',
		'\tif [ "$(cat /proc/sys/kernel/unprivileged_userns_clone 2>/dev/null)" = 0 ] ||',
		'\t\t[ "$(cat /proc/sys/user/max_user_namespaces 2>/dev/null)" = 0 ]; then',
		'\t\tchmod 4755 "$helper"',
		'\telse',
		'\t\tchmod 0755 "$helper"',
		'\tfi',
		'fi',
		'',
	].join('\n');
}

function postrm(stem) {
	return [
		'#!/bin/sh',
		'set -e',
		`target=/etc/apparmor.d/${stem}`,
		'if { [ "$1" = remove ] || [ "$1" = purge ]; } && [ -e "$target" ]; then',
		'\tif command -v apparmor_parser >/dev/null 2>&1 && [ -d /sys/kernel/security/apparmor ]; then',
		'\t\tapparmor_parser --remove "$target" >/dev/null 2>&1 || :',
		'\tfi',
		'\trm -f "$target"',
		'fi',
		'',
	].join('\n');
}

/** The Debian version of a product version: a pre-release sorts before its release with `~`. */
function debianVersion(version) {
	return version.replace('-', '~');
}

/** The total size in KiB of the regular files below `directory`, as `Installed-Size` counts it. */
async function installedSize(directory) {
	let total = 0;
	for (const entry of await readdir(directory, { recursive: true, withFileTypes: true }))
		if (entry.isFile())
			total += Math.ceil((await stat(path.join(entry.parentPath, entry.name))).size / 1024);
	return total;
}

/** Writes `text` at `filename` with `mode`, creating its folder. */
async function place(filename, text, mode = 0o644) {
	await mkdir(path.dirname(filename), { recursive: true, mode: 0o755 });
	await writeFile(filename, text);
	await chmod(filename, mode);
}

/**
 * Opens the copied application `directory` to every user, as an installed program is: its folders
 * and programs become 0755 and its other files 0644, owned by root once packed. The staged cohort
 * is private to its builder, which would leave the window's bundled Client to root alone.
 */
async function share(directory) {
	await chmod(directory, 0o755);
	const entries = await readdir(directory, { recursive: true, withFileTypes: true });
	await Promise.all(
		entries.map(async (entry) => {
			const entryPath = path.join(entry.parentPath, entry.name);
			if (entry.isDirectory()) await chmod(entryPath, 0o755);
			else if (entry.isFile())
				await chmod(entryPath, ((await stat(entryPath)).mode & 0o111) === 0 ? 0o644 : 0o755);
		}),
	);
}

/** Refuses an AppImage runtime that is not an ELF program for `machine`. */
async function checkRuntime(runtime, machine) {
	if (runtime === undefined || !path.isAbsolute(runtime))
		throw new Error('ORCHBEE_APPIMAGE_RUNTIME must name the AppImage runtime by an absolute path.');
	const file = await open(runtime, 'r');
	try {
		const { buffer } = await file.read(Buffer.alloc(20), 0, 20, 0);
		if (buffer.readUInt32BE(0) !== 0x7f454c46 || buffer.readUInt16LE(18) !== machine)
			throw new Error(`${runtime} is not an AppImage runtime for this platform.`);
	} finally {
		await file.close();
	}
}

/** Runs one packaging tool, saying which one is missing when it cannot start. */
function run(tool, arguments_) {
	try {
		execFileSync(tool, arguments_, { stdio: 'inherit' });
	} catch (error) {
		if (error?.code === 'ENOENT')
			throw new Error(`${tool} is not installed; the Linux packages need it.`, { cause: error });
		throw error;
	}
}

async function writeAppImage({ application, applicationName, file, machine, stem, workspace }) {
	const runtime = process.env['ORCHBEE_APPIMAGE_RUNTIME'];
	await checkRuntime(runtime, machine);
	const appDirectory = path.join(workspace, 'AppDir');
	await cp(application, appDirectory, { recursive: true, verbatimSymlinks: true });
	await share(appDirectory);
	await place(path.join(appDirectory, 'AppRun'), appRun(stem), 0o755);
	await place(
		path.join(appDirectory, `${stem}.desktop`),
		menuEntry({ applicationName, exec: `${stem}-desktop`, stem }),
	);
	const icon = path.join(application, 'resources', 'linux', 'icons', '256x256.png');
	await copyFile(icon, path.join(appDirectory, `${stem}.png`));
	await copyFile(icon, path.join(appDirectory, '.DirIcon'));
	const image = path.join(workspace, 'image.squashfs');
	run('mksquashfs', [
		appDirectory,
		image,
		'-root-owned',
		'-noappend',
		'-no-xattrs',
		'-comp',
		'zstd',
		'-quiet',
	]);
	await writeFile(file, Buffer.concat([await readFile(runtime), await readFile(image)]), {
		flag: 'wx',
	});
	await chmod(file, 0o755);
}

async function writeDebian({
	application,
	applicationName,
	debian,
	file,
	stem,
	version,
	workspace,
}) {
	const root = path.join(workspace, 'deb');
	await mkdir(root, { mode: 0o755 });
	await chmod(root, 0o755);
	const installed = path.join(root, 'opt', stem);
	await mkdir(path.dirname(installed), { recursive: true, mode: 0o755 });
	await cp(application, installed, { recursive: true, verbatimSymlinks: true });
	await share(installed);
	await place(
		path.join(root, 'usr', 'share', 'applications', `${stem}.desktop`),
		menuEntry({ applicationName, exec: `/opt/${stem}/${stem}-desktop`, stem }),
	);
	await Promise.all(
		ICON_SIZES.map(async (size) => {
			const icons = path.join(root, 'usr', 'share', 'icons', 'hicolor', `${size}x${size}`, 'apps');
			await mkdir(icons, { recursive: true, mode: 0o755 });
			await copyFile(
				path.join(application, 'resources', 'linux', 'icons', `${size}x${size}.png`),
				path.join(icons, `${stem}.png`),
			);
		}),
	);
	await place(path.join(root, 'usr', 'share', stem, 'apparmor-profile'), appArmorProfile(stem));
	const control = [
		`Package: ${stem}`,
		`Version: ${debianVersion(version)}`,
		`Architecture: ${debian}`,
		`Maintainer: ${MAINTAINER}`,
		`Installed-Size: ${await installedSize(root)}`,
		`Depends: ${DEPENDS.join(', ')}`,
		'Section: devel',
		'Priority: optional',
		`Homepage: ${HOMEPAGE}`,
		'Description: Modern, multiplexed terminal for running and orchestrating AI agents',
		'',
	].join('\n');
	await place(path.join(root, 'DEBIAN', 'control'), control);
	await place(path.join(root, 'DEBIAN', 'postinst'), postinst(stem), 0o755);
	await place(path.join(root, 'DEBIAN', 'postrm'), postrm(stem), 0o755);
	run('dpkg-deb', ['--root-owner-group', '-Zxz', '--build', root, file]);
}

/**
 * Writes the AppImage and the Debian package of the packaged application `applicationDirectory`
 * into `outputDirectory` as `<artifactName>-<version>-<platform>.AppImage` and `.deb`, working in
 * the empty folder `workspace`, and returns their paths.
 */
export async function writeLinuxPackages({
	applicationDirectory,
	applicationName,
	artifactName,
	outputDirectory,
	platform,
	stem,
	version,
	workspace,
}) {
	const architecture = ARCHITECTURES[platform];
	if (architecture === undefined)
		throw new Error(
			`The Linux packages are built for a Linux release platform, not ${String(platform)}.`,
		);
	const base = path.join(outputDirectory, `${artifactName}-${version}-${platform}`);
	const appImage = `${base}.AppImage`;
	const debianPackage = `${base}.deb`;
	await mkdir(path.join(workspace, 'appimage'));
	await mkdir(path.join(workspace, 'debian'));
	await writeAppImage({
		application: applicationDirectory,
		applicationName,
		file: appImage,
		machine: architecture.machine,
		stem,
		workspace: path.join(workspace, 'appimage'),
	});
	await writeDebian({
		application: applicationDirectory,
		applicationName,
		debian: architecture.debian,
		file: debianPackage,
		stem,
		version,
		workspace: path.join(workspace, 'debian'),
	});
	return [appImage, debianPackage];
}
