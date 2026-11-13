// The only switch between the "no release yet" and "released" install content. When a public
// pre-release exists, replace `release` with its version and the address of its Windows installer:
//
//   export const release: Release = {
//   	published: true,
//   	version: '0.1.0-beta.1',
//   	installerUrl: 'https://github.com/i-need-token/orchbee/releases/download/v0.1.0-beta.1/<installer>.exe',
//   };
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

export const release: Release = { published: false };

export function releaseNotesUrl(version: string): string {
	return `${REPOSITORY_URL}/releases/tag/v${version}`;
}
