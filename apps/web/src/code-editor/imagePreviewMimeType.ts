/** VS Code's image preview pattern, `*.{jpg,jpe,jpeg,png,bmp,gif,ico,webp,avif,svg}`, by type. */
const IMAGE_MIME_TYPES: ReadonlyMap<string, string> = new Map([
	['avif', 'image/avif'],
	['bmp', 'image/bmp'],
	['gif', 'image/gif'],
	['ico', 'image/x-icon'],
	['jpe', 'image/jpeg'],
	['jpeg', 'image/jpeg'],
	['jpg', 'image/jpeg'],
	['png', 'image/png'],
	['svg', 'image/svg+xml'],
	['webp', 'image/webp'],
]);

/**
 * The media type a file opens with in the image preview, or `undefined` for a file that opens as
 * text. Like VS Code's editor associations, it matches the name and ignores case.
 */
export function imagePreviewMimeType(path: string): string | undefined {
	const name = path.slice(Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')) + 1);
	const dot = name.lastIndexOf('.');
	return dot === -1 ? undefined : IMAGE_MIME_TYPES.get(name.slice(dot + 1).toLowerCase());
}
