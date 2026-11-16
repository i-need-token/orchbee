import { z } from 'zod';

/** What the browser's Local Font Access answered: one entry per installed face. */
const LocalFontFacesSchema = z.array(z.object({ family: z.string() }));

/** The installed font families, by name, or null when the browser refused or failed to list them. */
interface BrowserLocalFontCatalog {
	listFamilies(): Promise<readonly string[] | null>;
}

/**
 * The installed font families through the browser's Local Font Access (`queryLocalFonts`), or
 * undefined where the browser does not offer it. Desktop's Chromium offers it and grants the
 * permission; a browser asks the person the first time, so the first listing must follow a press.
 * A listing that succeeded is kept for the document's lifetime; a refusal is asked again next time.
 */
export function createBrowserLocalFontCatalog(
	host: object = globalThis,
): BrowserLocalFontCatalog | undefined {
	const query: unknown = Reflect.get(host, 'queryLocalFonts');
	if (typeof query !== 'function') return undefined;
	let listed: Promise<readonly string[]> | undefined;
	return {
		listFamilies: async () => {
			listed ??= (async (): Promise<readonly string[]> => {
				const faces = LocalFontFacesSchema.parse(await Reflect.apply(query, host, []));
				return [...new Set(faces.map((face) => face.family.trim()))]
					.filter((family) => family !== '')
					.sort((first, second) => first.localeCompare(second));
			})();
			try {
				return await listed;
			} catch {
				listed = undefined;
				return null;
			}
		},
	};
}
