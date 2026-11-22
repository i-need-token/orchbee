import { z } from 'zod';

/**
 * The preload posts this to its own window for each drop of files the page accepted, before the
 * page's own drop listener runs, because only the preload can read a dropped file's path.
 */
export const DESKTOP_FILE_DROP_WINDOW_MESSAGE = 'orchbee.desktop.file-drop';

/** The longest text one drop pastes: 1,024 paths of the longest Windows extended-length form. */
const DESKTOP_FILE_DROP_TEXT_LIMIT = 1024 * 32_768;

/** The dropped paths as the platform's own terminal types them, ready to paste. */
export const DesktopFileDropWindowMessageSchema = z.strictObject({
	text: z.string().min(1).max(DESKTOP_FILE_DROP_TEXT_LIMIT),
	type: z.literal(DESKTOP_FILE_DROP_WINDOW_MESSAGE),
});
