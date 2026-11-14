// The only switch between the "no release yet" and "released" install content. After a release is
// published, `pnpm website:release <version>` sets `release` to its version and Windows installer,
// together with the update feed under public/update/, so the download button and the update channel
// always name the same version. Keep the block's shape: the command rewrites it.
//
// Every section that offers Orchbee reads this value; nothing else changes.

export const REPOSITORY_URL = 'https://github.com/i-need-token/orchbee';

export const README_URL = `${REPOSITORY_URL}#readme`;

export const BUILD_GUIDE_URL = `${REPOSITORY_URL}#install`;

export const NPM_INSTALL_COMMAND = 'npm install -g orchbee@next';

export const SOURCE_COMMANDS = [
	'git clone https://github.com/i-need-token/orchbee.git',
	'cd orchbee',
	'corepack enable',
	'pnpm install',
] as const;

export type Release =
	| { readonly published: false }
	| { readonly published: true; readonly version: string; readonly installerUrl: string };

export const release: Release = {
	published: true,
	version: '0.1.0-beta.1',
	installerUrl: `${REPOSITORY_URL}/releases/download/v0.1.0-beta.1/Orchbee-0.1.0-beta.1-windows-x86_64-setup.exe`,
};

export function releaseNotesUrl(version: string): string {
	return `${REPOSITORY_URL}/releases/tag/v${version}`;
}
