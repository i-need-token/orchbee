// The only switch between the "no release yet" and "released" install content. After a release is
// published, `pnpm website:release <version>` sets `release` to its version and to the download of
// each platform the release published, together with the update feeds under public/update/, so the
// download buttons and the update channel always name the same version. Keep the block's shape:
// the command rewrites it.
//
// Every section that offers Orchbee reads this value; nothing else changes. A platform without a
// download here is not mentioned: the macOS downloads, the install script command and the Open
// Anyway steps render only while a macOS download is present.

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
	version: '0.1.0-beta.7',
	downloads: {
		'windows-x86_64': `${REPOSITORY_URL}/releases/download/v0.1.0-beta.7/Orchbee-0.1.0-beta.7-windows-x86_64-setup.exe`,
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

export function releaseNotesUrl(version: string): string {
	return `${REPOSITORY_URL}/releases/tag/v${version}`;
}
