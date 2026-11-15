// Re-theming a drawn window in the browser. A drawn terminal's palette is the public
// --ob-comp-website-terminal-* tokens; a theme travels in the page as two attributes, which a
// component writes at build time from terminalThemeData, and this module writes them to a window through the
// CSSOM, so the browser script carries no theme data and the page no style attribute.

// The page data of a theme, written by terminalThemeData's caller: the polarity, `dark` or
// `light`, and the palette as `name:#rrggbb` entries joined by `;`.
const TERMINAL_THEME_POLARITY = 'data-terminal-theme';
const TERMINAL_THEME_COLORS = 'data-terminal-colors';
/** The palette properties a theme sets, before each entry's name. */
const TERMINAL_PROPERTY_PREFIX = '--ob-comp-website-terminal-';

const ENTRY = /^([a-z0-9-]+):(#[0-9a-f]{6})$/u;

/**
 * Gives `window` the theme whose data `source` carries: it removes the palette properties a
 * previous theme wrote, writes this theme's, and sets the window's `data-theme` to the theme's
 * polarity so the product's own tokens follow. The default theme carries no colors, so applying it
 * leaves the stylesheet's values. A source without valid theme data changes nothing.
 */
export function applyTerminalTheme(window: HTMLElement, source: Element): void {
	const polarity = source.getAttribute(TERMINAL_THEME_POLARITY);
	if (polarity !== 'dark' && polarity !== 'light') return;
	// Backward, because removing a property shifts the ones after it.
	for (let index = window.style.length - 1; index >= 0; index -= 1) {
		const name = window.style.item(index);
		if (name.startsWith(TERMINAL_PROPERTY_PREFIX)) window.style.removeProperty(name);
	}
	for (const entry of (source.getAttribute(TERMINAL_THEME_COLORS) ?? '').split(';')) {
		const match = ENTRY.exec(entry);
		if (match?.[1] !== undefined && match[2] !== undefined)
			window.style.setProperty(`${TERMINAL_PROPERTY_PREFIX}${match[1]}`, match[2]);
	}
	window.setAttribute('data-theme', polarity);
}
