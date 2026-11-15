import { DESKTOP_WINDOW_CAPTION_WINDOW_MESSAGE } from '@orchbee/desktop-client-protocol';

/** The caption buttons' symbols read as the page's own primary icons in every theme. */
const CAPTION_SYMBOL_TOKEN = '--ob-sys-color-icon-primary';
/** The attributes that select the document's theme: its polarity and its named color theme. */
const THEME_ATTRIBUTES = ['data-theme', 'data-color-theme'];
/** A canvas keeps its previous fill when it cannot parse a color, so this fill marks a failed parse. */
const UNPARSED_FILL = '#010203';

function hexByte(value: number): string {
	return value.toString(16).padStart(2, '0');
}

/**
 * The symbol token's color under the document's current theme as opaque `#rrggbb`, or null when the
 * canvas cannot parse it. Tokens may be `oklch()` or `color-mix()`, which the caption's color parser
 * does not read, so the canvas converts them to sRGB; a translucent token keeps only its hue.
 */
function resolveSymbolColor(
	targetWindow: Window,
	context: CanvasRenderingContext2D,
): string | null {
	const value = targetWindow
		.getComputedStyle(targetWindow.document.documentElement)
		.getPropertyValue(CAPTION_SYMBOL_TOKEN)
		.trim();
	context.fillStyle = UNPARSED_FILL;
	context.fillStyle = value;
	if (value === '' || context.fillStyle === UNPARSED_FILL) return null;
	context.clearRect(0, 0, 1, 1);
	context.fillRect(0, 0, 1, 1);
	const [red, green, blue] = context.getImageData(0, 0, 1, 1).data;
	if (red === undefined || green === undefined || blue === undefined) return null;
	return `#${hexByte(red)}${hexByte(green)}${hexByte(blue)}`;
}

/**
 * Tells Main, through the preload, the color this document's theme gives the Windows caption
 * buttons' symbols: once now, and again whenever the theme attributes change, so the buttons follow
 * the application's theme, and the system's when the theme follows it, without a restart.
 */
export function followDesktopWindowCaption(targetWindow: Window): void {
	const { document } = targetWindow;
	const canvas = document.createElement('canvas');
	canvas.width = 1;
	canvas.height = 1;
	const context = canvas.getContext('2d', { willReadFrequently: true });
	if (context === null) return;
	let sent: string | null = null;
	const report = (): void => {
		const symbolColor = resolveSymbolColor(targetWindow, context);
		if (symbolColor === null || symbolColor === sent) return;
		sent = symbolColor;
		targetWindow.postMessage(
			{ symbolColor, type: DESKTOP_WINDOW_CAPTION_WINDOW_MESSAGE },
			targetWindow.location.origin,
		);
	};
	const observer = new MutationObserver(report);
	observer.observe(document.documentElement, { attributeFilter: THEME_ATTRIBUTES });
	report();
}
