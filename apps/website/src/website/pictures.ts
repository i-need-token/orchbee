import type { PictureFile } from './components/product-picture/ProductPicture';

// The files scripts/generate-website-pictures.mjs derives from resources/readme into
// public/pictures/, with the sizes it writes them at.
export const HERO_PICTURES = [
	{ src: '/pictures/hero-1600.webp', width: 1600, height: 882 },
	{ src: '/pictures/hero-800.webp', width: 800, height: 441 },
] as const satisfies readonly [PictureFile, ...PictureFile[]];
export const HERO_NARROW_PICTURE: PictureFile = {
	src: '/pictures/hero-agents-784.webp',
	width: 784,
	height: 830,
};
export const HERO_SIDEBAR_PICTURE = {
	src: '/pictures/hero-sidebar.webp',
	width: 256,
	height: 264,
} as const satisfies PictureFile;
export const DESKTOP_PICTURES = [
	{ src: '/pictures/desktop-1600.webp', width: 1600, height: 888 },
	{ src: '/pictures/desktop-800.webp', width: 800, height: 444 },
] as const satisfies readonly [PictureFile, ...PictureFile[]];
export const DESKTOP_LEFT_PICTURE: PictureFile = {
	src: '/pictures/desktop-left-808.webp',
	width: 808,
	height: 444,
};
export const MULTIPLEX_RECORDING = {
	src: '/pictures/multiplex.webm',
	poster: '/pictures/multiplex-still.webp',
	width: 1100,
	height: 324,
} as const;
export const MULTIPLEX_TUI_RECORDING = {
	src: '/pictures/multiplex-tui.webm',
	poster: '/pictures/multiplex-tui-still.webp',
	width: 546,
	height: 324,
} as const;
export const MULTIPLEX_DESKTOP_RECORDING = {
	src: '/pictures/multiplex-desktop.webm',
	poster: '/pictures/multiplex-desktop-still.webp',
	width: 550,
	height: 324,
} as const;
export const AGENTS_CLI_SHELL_RECORDING = {
	src: '/pictures/agents-cli-shell.webm',
	poster: '/pictures/agents-cli-shell-still.webp',
	width: 472,
	height: 605,
} as const;
export const AGENTS_CLI_RECORDING = {
	src: '/pictures/agents-cli.webm',
	poster: '/pictures/agents-cli-still.webp',
	width: 1100,
	height: 605,
} as const;
