import type { websiteShellMessages } from '@orchbee/i18n';

import type { IconName } from './components/icon/Icon';

// The only switch between the "no release yet" and "released" install content. After a release is
// published, `pnpm website:release <version>` sets `release` to its version and to the download of
// each platform the release published, together with the update feeds under public/update/, so the
// download buttons and the update channel always name the same version. Keep the block's shape:
// the command rewrites it.
//
// Every section that offers Orchbee or names its platforms reads this value. A platform appears on
// those surfaces only while the release publishes one of its installers; PLATFORMS below says which
// copy and which downloads each platform brings. Sentences that name platforms are copy, not data:
// apps/website/README.md lists the ones to update when a platform is added.

export const REPOSITORY_URL = 'https://github.com/i-need-token/orchbee';

export const BUILD_GUIDE_URL = `${REPOSITORY_URL}#install`;

export const NPM_INSTALL_COMMAND = 'npm install -g orchbee@next';

// The install script public/install.sh, served at the site root.
const INSTALL_SCRIPT_COMMAND = 'curl -fsSL https://orchbee.com/install.sh | sh';

const INSTALL_SCRIPT_DESKTOP_COMMAND = `${INSTALL_SCRIPT_COMMAND} -s -- --desktop`;

export const SOURCE_COMMANDS = [
	'git clone https://github.com/i-need-token/orchbee.git',
	'cd orchbee',
	'corepack enable',
	'pnpm install',
] as const;

// The downloads of each platform a release published: the Windows setup file, which every release
// has, each macOS disk image, and each Linux AppImage and Debian package. A platform added to
// PLATFORMS adds its download keys here too.
interface ReleaseDownloads {
	readonly 'windows-x86_64': string;
	readonly 'macos-aarch64'?: string;
	readonly 'macos-x86_64'?: string;
	readonly 'linux-x86_64-appimage'?: string;
	readonly 'linux-x86_64-deb'?: string;
	readonly 'linux-aarch64-appimage'?: string;
	readonly 'linux-aarch64-deb'?: string;
}

export type Release =
	| { readonly published: false }
	| { readonly published: true; readonly version: string; readonly downloads: ReleaseDownloads };

export const release: Release = {
	published: true,
	version: '0.1.0-beta.24',
	downloads: {
		'windows-x86_64': `${REPOSITORY_URL}/releases/download/v0.1.0-beta.24/Orchbee-0.1.0-beta.24-windows-x86_64-setup.exe`,
		'macos-aarch64': `${REPOSITORY_URL}/releases/download/v0.1.0-beta.24/Orchbee-0.1.0-beta.24-macos-aarch64.dmg`,
		'macos-x86_64': `${REPOSITORY_URL}/releases/download/v0.1.0-beta.24/Orchbee-0.1.0-beta.24-macos-x86_64.dmg`,
	},
};

/** Whether the release offers macOS, that is, publishes a macOS installer. */
export function offersMacos(offered: Release): boolean {
	return publishedPlatforms(offered).some(({ platform }) => platform.id === 'macos');
}

/** Whether the release offers Linux, that is, publishes a Linux installer. */
export function offersLinux(offered: Release): boolean {
	return publishedPlatforms(offered).some(({ platform }) => platform.id === 'linux');
}

/** The platforms besides Windows a release offers, by which a sentence that names platforms is chosen. */
export type OfferedPlatforms = 'windows' | 'macos' | 'linux' | 'all';

export function offeredPlatforms(offered: Release): OfferedPlatforms {
	const macos = offersMacos(offered);
	const linux = offersLinux(offered);
	if (macos && linux) return 'all';
	if (macos) return 'macos';
	return linux ? 'linux' : 'windows';
}

export function releaseNotesUrl(version: string): string {
	return `${REPOSITORY_URL}/releases/tag/v${version}`;
}

type ShellMessageKey = keyof typeof websiteShellMessages;

// One installer of a platform, with the copy that names it wherever a surface offers it. `download` is its key in
// ReleaseDownloads. `label` names it in the download dialog, `install` is its button in the install section, `cta`
// is the call to action's button when it is the platform's first published installer, and `link` is its text in the
// call to action's link line when it is one of the others (`cta` stands in when there is no `link`).
interface PlatformVariant {
	readonly download: keyof ReleaseDownloads;
	readonly label: ShellMessageKey;
	readonly install: ShellMessageKey;
	readonly cta: ShellMessageKey;
	readonly link?: ShellMessageKey;
}

// One other way to install that a platform needs: its label in the download dialog, its label there
// when another published platform offers the same command (the dialog lists it once), its label in
// the install section, and its command.
interface PlatformCommand {
	readonly label: ShellMessageKey;
	readonly sharedLabel?: ShellMessageKey;
	readonly install: ShellMessageKey;
	readonly command: string;
}

// A platform's rows in the install section: the downloads row (its title, its description and a button per
// installer) and, when the platform has commands, a commands row above it. Both show under the platform's tab,
// followed by the platform's note when it has one.
interface PlatformInstall {
	readonly title: ShellMessageKey;
	readonly detail: ShellMessageKey;
	readonly note?: ShellMessageKey;
	readonly commands?: { readonly title: ShellMessageKey; readonly detail: ShellMessageKey };
}

interface Platform {
	/** The platform's key on the page: its tab, its download group and its button attributes. */
	readonly id: string;
	readonly name: ShellMessageKey;
	readonly icon: IconName;
	/** Matches the browser's platform string, which names the visitor's operating system. */
	readonly visitor: RegExp;
	readonly variants: readonly PlatformVariant[];
	readonly commands: readonly PlatformCommand[];
	readonly install: PlatformInstall;
}

// Every surface that names a platform reads this catalog: the download dialog's groups, the call-to-action
// buttons and their links, and the install section's tabs and rows. A platform appears only while the release
// publishes one of its installers, in this order. The visitor's platform leads the dialog and the call to action
// (activateDownloadDialog and activateGetOrchbee move it first), and the install section opens on it
// (activateInstall).
//
// To add a platform, in this order (the "Release state" section of apps/website/README.md lists the same steps):
// 1. Its download key: in ReleaseDownloads above, and in the `downloads` table of scripts/website-release.mjs,
//    which refuses a release that publishes a platform missing from that table. `pnpm website:release` then
//    writes its download into the release block under that key, once a release publishes that installer.
// 2. Its entry here: `id`, `name` (its copy key), `icon`, the `visitor` pattern (it must not match Android, whose
//    platform string also starts with "Linux"), `variants` (one per installer: its `download` key and the copy keys
//    `label`, `install` and `cta`; `link` names a non-first installer in the call to action's link line, and `cta`
//    stands in when there is no `link`), `commands` (other ways to install, if any), and `install` (the copy of its
//    install rows: `title`, `detail` and `note`, plus `commands` when it has commands).
// 3. Its icon: a PATHS entry in components/icon/Icon.tsx, under the name `icon` gives.
// 4. Its copy keys: every key the entry references, in packages/i18n/src/descriptors/website-shell.ts with a
//    description for translators, and in both locales (en-US and zh-CN). The typecheck names a key that a locale
//    lacks or the catalog does not know; then run `pnpm --filter @orchbee/i18n build`.
// 5. The sentences that name platforms. They are copy, not generated: the install lead and start note
//    (installLead* and installStartNote* in website-home), the home hero's line of facts and the platforms answer
//    among the home questions (facts* and faqPlatformsAnswer* in website-home), and the desktop sections that
//    describe macOS and Linux (DesktopBackground, DesktopHighlights, DesktopKeys). They read offeredPlatforms,
//    offersMacos or offersLinux, so each names a platform only while it is published.
// 6. The checks: `pnpm --filter @orchbee/website typecheck` and `pnpm --filter @orchbee/website test`. Then the
//    call to action and the install section at 400 and 1440 pixels in both locales, with a visitor of each platform.
// Nothing else needs a change: the call-to-action buttons, the download dialog's groups, the install tabs and rows,
// and the visitor ordering all read PLATFORMS.
const PLATFORMS = [
	{
		id: 'windows',
		name: 'platformWindows',
		icon: 'windows',
		visitor: /Win/iu,
		variants: [
			{
				download: 'windows-x86_64',
				label: 'downloadAction',
				install: 'downloadForWindows',
				cta: 'downloadForWindows',
			},
		],
		commands: [],
		install: {
			title: 'installWindowsTitle',
			detail: 'installWindowsDetail',
			note: 'installWindowsNote',
		},
	},
	{
		id: 'macos',
		name: 'platformMacos',
		icon: 'apple',
		visitor: /Mac|iPhone|iPad|iPod/iu,
		variants: [
			{
				download: 'macos-aarch64',
				label: 'downloadAppleSilicon',
				install: 'installDownloadAppleSilicon',
				cta: 'downloadForMacAppleSilicon',
			},
			{
				download: 'macos-x86_64',
				label: 'downloadIntel',
				install: 'installDownloadIntel',
				cta: 'downloadForMac',
				link: 'downloadForMacIntel',
			},
		],
		commands: [
			{
				label: 'downloadScriptDesktop',
				sharedLabel: 'downloadScriptDesktopShared',
				install: 'installScriptDesktop',
				command: INSTALL_SCRIPT_DESKTOP_COMMAND,
			},
			{
				label: 'downloadScriptCli',
				sharedLabel: 'downloadScriptCliShared',
				install: 'installScriptCli',
				command: INSTALL_SCRIPT_COMMAND,
			},
		],
		install: {
			title: 'installMacosTitle',
			detail: 'installMacosDetail',
			commands: { title: 'installMacosScriptTitle', detail: 'installMacosScriptDetail' },
		},
	},
	{
		id: 'linux',
		name: 'platformLinux',
		icon: 'linux',
		// A desktop browser's platform string, or an X11 user agent. Android's platform string also starts
		// with "Linux" but names an ARM processor, as an ARM desktop's does, so neither is matched.
		visitor: /^Linux x86_64|X11/u,
		variants: [
			{
				download: 'linux-x86_64-appimage',
				label: 'downloadAppImageX64',
				install: 'installDownloadAppImageX64',
				cta: 'downloadForLinux',
			},
			{
				download: 'linux-x86_64-deb',
				label: 'downloadDebX64',
				install: 'installDownloadDebX64',
				cta: 'downloadForLinuxDeb',
			},
			{
				download: 'linux-aarch64-appimage',
				label: 'downloadAppImageArm',
				install: 'installDownloadAppImageArm',
				cta: 'downloadForLinuxArmAppImage',
			},
			{
				download: 'linux-aarch64-deb',
				label: 'downloadDebArm',
				install: 'installDownloadDebArm',
				cta: 'downloadForLinuxArmDeb',
			},
		],
		commands: [
			{
				label: 'downloadScriptDesktopLinux',
				sharedLabel: 'downloadScriptDesktopShared',
				install: 'installScriptDesktop',
				command: INSTALL_SCRIPT_DESKTOP_COMMAND,
			},
			{
				label: 'downloadScriptCliLinux',
				sharedLabel: 'downloadScriptCliShared',
				install: 'installScriptCli',
				command: INSTALL_SCRIPT_COMMAND,
			},
		],
		install: {
			title: 'installLinuxTitle',
			detail: 'installLinuxDetail',
			commands: { title: 'installLinuxScriptTitle', detail: 'installLinuxScriptDetail' },
		},
	},
] as const satisfies readonly Platform[];

const catalog: readonly Platform[] = PLATFORMS;

export type PlatformId = (typeof PLATFORMS)[number]['id'];

interface PublishedVariant extends PlatformVariant {
	readonly url: string;
}

interface PublishedPlatform {
	readonly platform: Platform;
	readonly variants: readonly PublishedVariant[];
}

/** The platforms a release published, in catalog order, each with its published installers; empty before a release. */
export function publishedPlatforms(offered: Release): readonly PublishedPlatform[] {
	if (!offered.published) return [];
	return catalog.flatMap((platform) => {
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

/**
 * Whether the visitor is on a phone or a tablet, where Orchbee does not run: an iPhone, iPad, iPod or
 * Android device by its user agent, or an iPad that reports a Mac platform but has a touch screen.
 */
export function visitorHandheld({
	userAgent,
	platform,
	maxTouchPoints,
}: {
	readonly userAgent: string;
	readonly platform: string;
	readonly maxTouchPoints: number;
}): boolean {
	return (
		/iPhone|iPad|iPod|Android/iu.test(userAgent) ||
		(platform.startsWith('Mac') && maxTouchPoints > 1)
	);
}

/**
 * The items with the visitor's platform first and the others after them in their own order. Without a visitor
 * platform, or when no item belongs to it, the items keep their order.
 */
export function visitorFirst<T>(
	items: readonly T[],
	platformOf: (item: T) => string | undefined,
	visitor: PlatformId | undefined,
): readonly T[] {
	if (visitor === undefined) return items;
	return [
		...items.filter((item) => platformOf(item) === visitor),
		...items.filter((item) => platformOf(item) !== visitor),
	];
}
