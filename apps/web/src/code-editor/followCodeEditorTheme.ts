import { codeEditorThemeData, type CodeEditorThemeColors } from './codeEditorThemeData';
import type { editor } from './monaco-entry';

/** The attributes that select the document's theme: its polarity and its named color theme. */
const THEME_ATTRIBUTES = ['data-theme', 'data-color-theme'];
/** Monaco's theme service is page-wide, so one theme name serves every editor. */
const THEME_NAME = 'orchbee';
/** The surface the editor paints, under every color it reads as opaque. */
const CANVAS_TOKEN = '--ob-sys-color-surface-canvas';
/** A canvas keeps its previous fill when it cannot parse a color, so this fill marks a failed parse. */
const UNPARSED_FILL = '#010203';

type ThemeContext = OffscreenCanvasRenderingContext2D;

function hexByte(value: number): string {
	return value.toString(16).padStart(2, '0');
}

/**
 * Reads the document's tokens through a 1×1 canvas, which paints `oklch()` and `color-mix()`
 * values that Monaco's hex parser does not read and returns their sRGB bytes.
 */
function documentColors(targetWindow: Window, context: ThemeContext): CodeEditorThemeColors {
	const style = targetWindow.getComputedStyle(targetWindow.document.documentElement);
	const fill = (token: string): string | null => {
		const value = style.getPropertyValue(token).trim();
		context.fillStyle = UNPARSED_FILL;
		context.fillStyle = value;
		if (value === '' || context.fillStyle === UNPARSED_FILL) return null;
		return value;
	};
	const paint = (fills: readonly string[]): readonly number[] => {
		context.clearRect(0, 0, 1, 1);
		for (const value of fills) {
			context.fillStyle = value;
			context.fillRect(0, 0, 1, 1);
		}
		return [...context.getImageData(0, 0, 1, 1).data];
	};
	return {
		color(token) {
			const value = fill(token);
			if (value === null) return null;
			const [red = 0, green = 0, blue = 0, alpha = 0] = paint([value]);
			const rgb = `#${hexByte(red)}${hexByte(green)}${hexByte(blue)}`;
			return alpha === 255 ? rgb : `${rgb}${hexByte(alpha)}`;
		},
		onCanvas(token) {
			const canvas = fill(CANVAS_TOKEN);
			const value = fill(token);
			if (canvas === null || value === null) return null;
			const [red = 0, green = 0, blue = 0] = paint([canvas, value]);
			return `#${hexByte(red)}${hexByte(green)}${hexByte(blue)}`;
		},
	};
}

/**
 * Gives Monaco the document's theme: once now, and again whenever the theme attributes change, so
 * every editor follows the application's appearance and named color theme without a reload. A
 * host without `OffscreenCanvas`, such as a test document, keeps Monaco's default theme.
 */
export function followCodeEditorTheme(
	targetWindow: Window,
	runtime: { readonly editor: Pick<typeof editor, 'defineTheme' | 'setTheme'> },
): void {
	if (!('OffscreenCanvas' in globalThis)) return;
	const context = new OffscreenCanvas(1, 1).getContext('2d', { willReadFrequently: true });
	if (context === null) return;
	const root = targetWindow.document.documentElement;
	const apply = (): void => {
		const dark = root.dataset['theme'] === 'dark';
		runtime.editor.defineTheme(
			THEME_NAME,
			codeEditorThemeData(documentColors(targetWindow, context), dark),
		);
		runtime.editor.setTheme(THEME_NAME);
	};
	new MutationObserver(apply).observe(root, { attributeFilter: THEME_ATTRIBUTES });
	apply();
}
