import type { websiteShellMessages } from '@orchbee/i18n';

import type { IconName } from './components/icon/Icon';

// The only switch between the "no release yet" and "released" install content. After a release is
// published, `pnpm website:release <version>` sets `release` to its version and to the download of
// each platform the release published, together with the update feeds under public/update/, so the
// download buttons and the update channel always name the same version. Keep the block's shape:
// the command rewrites it.
//
// Every section that offers Orchbee or names its platforms reads this value; nothing else changes.
// A platform without a download here is not mentioned: the macOS downloads, the install script
// command, the Open Anyway steps, the Mac call to action and the copy that names macOS render only
// while a macOS download is present.

export const REPOSITORY_URL = 'https://github.com/i-need-token/orchbee';

export const BUILD_GUIDE_URL = `${REPOSITORY_URL}#install`;

export const NPM_INSTALL_COMMAND = 'npm install -g orchbee@next';

// The install script public/install.sh, served at the site root.
export const INSTALL_SCRIPT_COMMAND = 'curl -fsSL https://orchbee.com/install.sh | sh';

export const INSTALL_SCRIPT_DESKTOP_COMMAND = `${INSTALL_SCRIPT_COMMAND} -s -- --desktop`;

export const SOURCE_COMMANDS = [
	'git clone https://github.com/i-need-token/orchbee.git',
	'cd orchbee',
	'corepack enable',
	'pnpm install',
] as const;

// The download of each platform a release published: the Windows setup file, which every release
// has, and each macOS disk image.
export interface ReleaseDownloads {
	readonly 'windows-x86_64': string;
	readonly 'macos-aarch64'?: string;
	readonly 'macos-x86_64'?: string;
}

export type Release =
	| { readonly published: false }
	| { readonly published: true; readonly version: string; readonly downloads: ReleaseDownloads };

export const release: Release = {
	published: true,
	version: '0.1.0-beta.19',
	downloads: {
		'windows-x86_64': `${REPOSITORY_URL}/releases/download/v0.1.0-beta.19/Orchbee-0.1.0-beta.19-windows-x86_64-setup.exe`,
		'macos-aarch64': `${REPOSITORY_URL}/releases/download/v0.1.0-beta.19/Orchbee-0.1.0-beta.19-macos-aarch64.dmg`,
		'macos-x86_64': `${REPOSITORY_URL}/releases/download/v0.1.0-beta.19/Orchbee-0.1.0-beta.19-macos-x86_64.dmg`,
	},
};

export type MacosPlatform = 'macos-aarch64' | 'macos-x86_64';

/** The macOS disk images of a release, Apple silicon first; empty before a macOS release. */
export function macosDownloads(
	downloads: ReleaseDownloads,
): readonly { readonly platform: MacosPlatform; readonly url: string }[] {
	return (['macos-aarch64', 'macos-x86_64'] as const).flatMap((platform) => {
		const url = downloads[platform];
		return url === undefined ? [] : [{ platform, url }];
	});
}

/** Whether the release offers macOS, that is, publishes at least one macOS disk image. */
export function offersMacos(offered: Release): boolean {
	return offered.published && macosDownloads(offered.downloads).length > 0;
}

export function releaseNotesUrl(version: string): string {
	return `${REPOSITORY_URL}/releases/tag/v${version}`;
}

type ShellMessageKey = keyof typeof websiteShellMessages;

export type PlatformId = 'windows' | 'macos';

// One installer of a platform: the download key it reads from ReleaseDownloads and its button's copy.
interface PlatformVariant {
	readonly download: keyof ReleaseDownloads;
	readonly label: ShellMessageKey;
}

// One other way to install that a platform needs: its label and its command.
interface PlatformCommand {
	readonly label: ShellMessageKey;
	readonly command: string;
}

interface Platform {
	readonly id: PlatformId;
	readonly name: ShellMessageKey;
	readonly icon: IconName;
	/** Matches the browser's platform string, which names the visitor's operating system. */
	readonly visitor: RegExp;
	readonly variants: readonly PlatformVariant[];
	readonly commands: readonly PlatformCommand[];
}

// The platforms the download dialog groups its installers by, in the order it lists them to a visitor who
// matches none of them. A platform appears in the dialog only while the release publishes one of its variants.
// To add a platform: add its download keys to ReleaseDownloads and to the release script's table, add an entry
// here, add its icon to components/icon/Icon.tsx, and add its copy in both locales.
const PLATFORMS: readonly Platform[] = [
	{
		id: 'windows',
		name: 'platformWindows',
		icon: 'windows',
		visitor: /Win/iu,
		variants: [{ download: 'windows-x86_64', label: 'downloadAction' }],
		commands: [],
	},
	{
		id: 'macos',
		name: 'platformMacos',
		icon: 'apple',
		visitor: /Mac|iPhone|iPad|iPod/iu,
		variants: [
			{ download: 'macos-aarch64', label: 'downloadAppleSilicon' },
			{ download: 'macos-x86_64', label: 'downloadIntel' },
		],
		commands: [
			{ label: 'downloadScriptDesktop', command: INSTALL_SCRIPT_DESKTOP_COMMAND },
			{ label: 'downloadScriptCli', command: INSTALL_SCRIPT_COMMAND },
		],
	},
];

interface PublishedVariant extends PlatformVariant {
	readonly url: string;
}

export interface PublishedPlatform {
	readonly platform: Platform;
	readonly variants: readonly PublishedVariant[];
}

/** The platforms a release published, in catalog order, each with its published variants; empty before a release. */
export function publishedPlatforms(offered: Release): readonly PublishedPlatform[] {
	if (!offered.published) return [];
	return PLATFORMS.flatMap((platform) => {
		const variants = platform.variants.flatMap((variant) => {
			const url = offered.downloads[variant.download];
			return url === undefined ? [] : [{ ...variant, url }];
		});
		return variants.length === 0 ? [] : [{ platform, variants }];
	});
}

/** The platform whose pattern matches a browser's platform string, or undefined when none does. */
export function visitorPlatform(platformString: string): PlatformId | undefined {
	return PLATFORMS.find((platform) => platform.visitor.test(platformString))?.id;
}
