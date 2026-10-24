import { z } from 'zod';
import {
	SiteIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchPanelIdSchema,
	TerminalPresentationIdSchema,
	WorkbenchDirectoryPathSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchPresentationBindingSchema,
} from './workbench-semantic-contract';
import {
	WorkbenchClientIdSchema,
	ClientViewSequenceSchema,
} from './workbench-client-view-contract';
import {
	GRAPHICS_SET_BYTES,
	GRAPHICS_STREAM_BYTES,
	GRAPHICS_RESIDENT_BYTES,
	GRAPHICS_CHUNK_BYTES,
	GRAPHICS_REQUEST_BYTES,
	GRAPHICS_RESULT_BYTES,
	GraphicsLayerIdSchema,
	GraphicsDimensionsSchema,
	GraphicsPlacementSchema,
	GraphicsCapabilitiesSchema,
	GraphicsDamageSchema,
	graphicsFrameSchema,
	graphicsBase64Bytes,
	graphicsRgbaBytes,
} from './workbench-graphics-image-contract';
const encoder = new TextEncoder();
const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	siteId: SiteIdSchema,
};
const positiveRevision = WorkbenchRevisionSchema.refine((value) => BigInt(value) > 0n);
const PaneGraphicsTargetSchema = z.strictObject({
	panelId: WorkbenchPanelIdSchema,
	presentationId: TerminalPresentationIdSchema.nullable(),
});
const resolved = z.strictObject({
	panelId: WorkbenchPanelIdSchema,
	presentationId: TerminalPresentationIdSchema,
});
const layer = { layerId: GraphicsLayerIdSchema };
const zIndex = z.number().int().min(-2147483648).max(2147483647);
const stream = { streamId: WorkbenchRequestIdSchema };
const file = WorkbenchDirectoryPathSchema;
const PaneGraphicsLayerSchema = z
	.strictObject({
		...layer,
		zIndex,
		...GraphicsDimensionsSchema,
		placement: GraphicsPlacementSchema,
		resourceRevision: positiveRevision,
		streamId: WorkbenchRequestIdSchema.nullable(),
	})
	.refine((value) => graphicsRgbaBytes(value.width, value.height) !== null);
const PaneGraphicsFailureSchema = z.enum([
	'denied',
	'invalid-intent',
	'site-mismatch',
	'unavailable',
	'busy',
	'not-found',
	'conflict',
	'source-changed',
	'source-unavailable',
	'invalid-image',
	'image-too-large',
	'layer-limit',
	'memory-limit',
	'stale-resource',
	'invalid-sequence',
	'stream-closed',
	'physical-unknown',
	'revision-exhausted',
	'invalid-file',
	'cell-size-unavailable',
]);
const streamImage = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('inline'), image: graphicsFrameSchema(GRAPHICS_STREAM_BYTES) }),
	z
		.strictObject({
			kind: z.literal('file'),
			format: z.enum(['rgba', 'bgra']),
			...GraphicsDimensionsSchema,
			path: file,
			placement: GraphicsPlacementSchema,
		})
		.refine((value) => {
			const bytes = graphicsRgbaBytes(value.width, value.height);
			return bytes !== null && bytes <= GRAPHICS_STREAM_BYTES;
		}),
]);
const action = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('observe'),
		clientId: WorkbenchClientIdSchema,
		heartbeat: ClientViewSequenceSchema,
		observation: z
			.strictObject({
				workbenchRevision: WorkbenchRevisionSchema,
				capabilities: GraphicsCapabilitiesSchema,
				complete: z.boolean(),
				panes: z
					.array(
						z.strictObject({
							target: resolved,
							binding: WorkbenchPresentationBindingSchema.nullable(),
							drawnRevision: WorkbenchRevisionSchema.nullable(),
						}),
					)
					.max(64),
			})
			.refine(
				(value) =>
					new Set(value.panes.map((pane) => pane.target.panelId)).size === value.panes.length,
			),
	}),
	z.strictObject({
		kind: z.literal('info'),
		target: PaneGraphicsTargetSchema,
		clientId: WorkbenchClientIdSchema.nullable(),
	}),
	z.strictObject({ kind: z.literal('catalog'), target: PaneGraphicsTargetSchema }),
	z.strictObject({
		kind: z.literal('set'),
		target: PaneGraphicsTargetSchema,
		...layer,
		zIndex,
		image: graphicsFrameSchema(GRAPHICS_SET_BYTES),
	}),
	z.strictObject({ kind: z.literal('clear'), target: PaneGraphicsTargetSchema, ...layer }),
	z.strictObject({
		kind: z.literal('pixels'),
		target: PaneGraphicsTargetSchema,
		...layer,
		resourceRevision: positiveRevision,
		offset: z.number().int().min(0).max(4294967295).multipleOf(4),
		maximumBytes: z.number().int().min(4).max(GRAPHICS_CHUNK_BYTES).multipleOf(4),
	}),
	z.strictObject({
		kind: z.literal('stream-open'),
		...stream,
		target: PaneGraphicsTargetSchema,
		...layer,
		zIndex,
	}),
	z
		.strictObject({
			kind: z.literal('stream-frame'),
			...stream,
			sequence: positiveRevision,
			revision: WorkbenchRevisionSchema,
			image: streamImage,
			damage: z.array(GraphicsDamageSchema).max(64),
		})
		.refine((value) => {
			const image = value.image.kind === 'inline' ? value.image.image : value.image;
			return value.damage.every(
				(damage) =>
					damage.x + damage.width <= image.width && damage.y + damage.height <= image.height,
			);
		}),
	z.strictObject({ kind: z.literal('stream-close'), ...stream }),
]);
export const PaneGraphicsRequestSchema = z
	.strictObject({ kind: z.literal('workbench.pane-graphics'), ...correlation, action })
	.refine((value) => encoder.encode(JSON.stringify(value)).byteLength <= GRAPHICS_REQUEST_BYTES)
	.readonly();
const pixels = z
	.strictObject({
		kind: z.literal('pixels'),
		target: resolved,
		...layer,
		resourceRevision: positiveRevision,
		...GraphicsDimensionsSchema,
		offset: z.number().int().min(0).max(4294967295).multipleOf(4),
		totalBytes: z
			.number()
			.int()
			.min(4)
			.max(32 * 1024 * 1024),
		dataBase64: z
			.string()
			.min(4)
			.max(Math.ceil(GRAPHICS_CHUNK_BYTES / 3) * 4),
		complete: z.boolean(),
	})
	.refine((value) => {
		const bytes = graphicsBase64Bytes(value.dataBase64);
		return (
			bytes !== null &&
			bytes > 0 &&
			bytes <= GRAPHICS_CHUNK_BYTES &&
			bytes % 4 === 0 &&
			value.totalBytes === graphicsRgbaBytes(value.width, value.height) &&
			value.offset + bytes <= value.totalBytes &&
			value.complete === (value.offset + bytes === value.totalBytes)
		);
	});
const info = z
	.strictObject({
		kind: z.literal('info'),
		target: resolved,
		revision: WorkbenchRevisionSchema,
		layerCount: z.number().int().min(0).max(16),
		clientId: WorkbenchClientIdSchema.nullable(),
		capabilities: GraphicsCapabilitiesSchema.nullable(),
		paneVisible: z.boolean().nullable(),
		drawnRevision: WorkbenchRevisionSchema.nullable(),
		fileFrameDirectory: file.nullable(),
	})
	.refine(
		(value) =>
			(value.clientId !== null ||
				(value.capabilities === null &&
					value.paneVisible === null &&
					value.drawnRevision === null)) &&
			(value.drawnRevision === null || BigInt(value.drawnRevision) <= BigInt(value.revision)),
	);
const outcome = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('observed'),
		clientId: WorkbenchClientIdSchema,
		heartbeat: ClientViewSequenceSchema,
	}),
	info,
	z
		.strictObject({
			kind: z.literal('catalog'),
			target: resolved,
			revision: WorkbenchRevisionSchema,
			layers: z.array(PaneGraphicsLayerSchema).max(16),
		})
		.refine(
			(value) =>
				new Set(value.layers.map((item) => item.layerId)).size === value.layers.length &&
				value.layers.every((item) => BigInt(item.resourceRevision) <= BigInt(value.revision)) &&
				value.layers.reduce(
					(sum, item) =>
						sum + (graphicsRgbaBytes(item.width, item.height) ?? GRAPHICS_RESIDENT_BYTES + 1),
					0,
				) <= GRAPHICS_RESIDENT_BYTES,
		),
	pixels,
	z
		.strictObject({
			kind: z.literal('applied'),
			target: resolved,
			...layer,
			revision: WorkbenchRevisionSchema,
			changed: z.boolean(),
		})
		.refine((value) => !value.changed || BigInt(value.revision) > 0n),
	z.strictObject({
		kind: z.literal('stream-opened'),
		...stream,
		target: resolved,
		...layer,
		zIndex,
		revision: positiveRevision,
		fileFrameDirectory: file.nullable(),
	}),
	z.strictObject({
		kind: z.literal('frame-applied'),
		...stream,
		target: resolved,
		...layer,
		sequence: positiveRevision,
		frameRevision: WorkbenchRevisionSchema,
		revision: positiveRevision,
		changed: z.boolean(),
	}),
	z.strictObject({
		kind: z.literal('closed'),
		...stream,
		revision: WorkbenchRevisionSchema,
		removed: z.boolean(),
	}),
	z.strictObject({ kind: z.literal('failed'), reason: PaneGraphicsFailureSchema }),
]);
export const PaneGraphicsResultSchema = z
	.strictObject({ kind: z.literal('workbench.pane-graphics.result'), ...correlation, outcome })
	.refine((value) => encoder.encode(JSON.stringify(value)).byteLength <= GRAPHICS_RESULT_BYTES)
	.readonly();
