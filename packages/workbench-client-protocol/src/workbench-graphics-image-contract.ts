import { z } from 'zod';

export const GRAPHICS_SET_BYTES = 512 * 1024;
export const GRAPHICS_STREAM_BYTES = 16 * 1024 * 1024;
export const GRAPHICS_DECODED_BYTES = 32 * 1024 * 1024;
export const GRAPHICS_RESIDENT_BYTES = 64 * 1024 * 1024;
export const GRAPHICS_CHUNK_BYTES = 256 * 1024;
export const GRAPHICS_REQUEST_BYTES = 24 * 1024 * 1024;
export const GRAPHICS_RESULT_BYTES = 512 * 1024;
export const GraphicsLayerIdSchema = z
	.string()
	.min(1)
	.max(80)
	.regex(/^[A-Za-z0-9_.-]+$/u);
export const GraphicsFormatSchema = z.enum(['png', 'rgb', 'rgba', 'bgra']);
export const GraphicsPlacementSchema = z.strictObject({
	viewportColumn: z.number().int().min(-2147483648).max(2147483647),
	viewportRow: z.number().int().min(-2147483648).max(2147483647),
	gridColumns: z.number().int().min(0).max(4294967295),
	gridRows: z.number().int().min(0).max(4294967295),
});
const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
/** Check canonical padding before decoding, without allocating a second copy of a large image. */
export function graphicsBase64Bytes(value: string): number | null {
	if (value.length === 0 || value.length % 4 !== 0) return null;
	const padding = value.endsWith('==') ? 2 : Number(value.endsWith('='));
	for (let i = 0; i < value.length - padding; i++)
		if (!alphabet.includes(value.charAt(i))) return null;
	if (padding === 2 && (alphabet.indexOf(value.charAt(value.length - 3)) & 15) !== 0) return null;
	if (padding === 1 && (alphabet.indexOf(value.charAt(value.length - 2)) & 3) !== 0) return null;
	return (value.length / 4) * 3 - padding;
}
export const graphicsRgbaBytes = (width: number, height: number): number | null => {
	if (
		!Number.isInteger(width) ||
		!Number.isInteger(height) ||
		width < 1 ||
		height < 1 ||
		width > 4096 ||
		height > 4096 ||
		width * height * 4 > GRAPHICS_DECODED_BYTES
	)
		return null;
	return width * height * 4;
};
export const GraphicsDimensionsSchema = {
	width: z.number().int().min(1).max(4096),
	height: z.number().int().min(1).max(4096),
};
const graphicsFrameShape = z.strictObject({
	format: GraphicsFormatSchema,
	...GraphicsDimensionsSchema,
	dataBase64: z.string(),
	placement: GraphicsPlacementSchema,
});
export const graphicsFrameSchema = (maximum: number): typeof graphicsFrameShape =>
	graphicsFrameShape
		.extend({
			dataBase64: z
				.string()
				.min(4)
				.max(Math.ceil(maximum / 3) * 4),
		})
		.refine((frame) => {
			const length = graphicsBase64Bytes(frame.dataBase64);
			const rgba = graphicsRgbaBytes(frame.width, frame.height);
			return (
				rgba !== null &&
				length !== null &&
				length <= maximum &&
				(frame.format === 'png'
					? length >= 33
					: length === (frame.format === 'rgb' ? (rgba / 4) * 3 : rgba))
			);
		});
export const GraphicsCapabilitiesSchema = z
	.strictObject({
		transport: z.enum(['cell-raster', 'kitty', 'sixel']),
		cellPixels: z
			.strictObject({
				width: z.number().int().min(1).max(65535),
				height: z.number().int().min(1).max(65535),
			})
			.nullable(),
		pixelMouse: z.boolean(),
	})
	.refine(
		(capabilities) =>
			!capabilities.pixelMouse ||
			(capabilities.transport !== 'cell-raster' && capabilities.cellPixels !== null),
	);
export const GraphicsDamageSchema = z.strictObject({
	x: z.number().int().min(0).max(4294967295),
	y: z.number().int().min(0).max(4294967295),
	width: z.number().int().min(1).max(4294967295),
	height: z.number().int().min(1).max(4294967295),
});
