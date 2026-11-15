import { z } from 'zod';

/**
 * The page posts its caption symbol color to its own window, and the preload forwards it to Main
 * on this channel, so the Windows caption buttons draw in the page's theme.
 */
export const DESKTOP_WINDOW_CAPTION_WINDOW_MESSAGE = 'orchbee.desktop.window-caption';
export const DESKTOP_WINDOW_CAPTION_IPC_CHANNEL = 'orchbee.desktop.window-caption';

/** An opaque sRGB color as `#rrggbb`, the one form Main hands to the native caption buttons. */
export const DesktopWindowCaptionColorSchema = z.string().regex(/^#[0-9a-f]{6}$/u);

/** The color the page's theme gives the caption buttons' symbols, sent whenever its theme changes. */
export const DesktopWindowCaptionWindowMessageSchema = z.strictObject({
	symbolColor: DesktopWindowCaptionColorSchema,
	type: z.literal(DESKTOP_WINDOW_CAPTION_WINDOW_MESSAGE),
});
