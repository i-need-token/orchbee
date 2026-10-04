import { z } from 'zod';
import {
	WorkbenchTerminalContextIdSchema,
	WorkbenchTabIdSchema,
	WorkbenchPanelIdSchema,
	WorkbenchSplitIdSchema,
	WorkbenchPanelSlotIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchSplitRatioSchema,
	WORKBENCH_MAXIMUM_PANEL_NODES_PER_CONTEXT,
	WORKBENCH_MAXIMUM_PANEL_NODE_DEPTH,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES = 1024 * 1024;
const cell = z.number().int().min(0).max(65535);
export const ClientCellRectSchema = z
	.strictObject({
		x: cell,
		y: cell,
		width: cell,
		height: cell,
	})
	.refine(
		(rect) => rect.x + rect.width <= 65535 && rect.y + rect.height <= 65535,
		'Cell extent overflow',
	)
	.readonly();
export const ClientPanelDirectionSchema = z.enum(['left', 'right', 'up', 'down']);
export const ClientSplitRatioSchema = WorkbenchSplitRatioSchema;
export const ClientResizeAmountSchema = z.number().min(0).max(0.5);
export const ClientSplitPathSchema = z
	.array(z.union([z.literal(0), z.literal(1)]))
	.max(WORKBENCH_MAXIMUM_PANEL_NODE_DEPTH)
	.readonly();
const ClientSplitChangeSchema = z
	.strictObject({
		splitId: WorkbenchSplitIdSchema,
		firstSlotId: WorkbenchPanelSlotIdSchema,
		secondSlotId: WorkbenchPanelSlotIdSchema,
		previousRatio: ClientSplitRatioSchema,
		ratio: ClientSplitRatioSchema,
	})
	.refine((split) => split.firstSlotId !== split.secondSlotId, 'Split slots must differ')
	.readonly();
export const ClientLayoutAdjustmentSchema = z
	.strictObject({
		terminalContextId: WorkbenchTerminalContextIdSchema,
		tabId: WorkbenchTabIdSchema,
		workbenchRevision: WorkbenchRevisionSchema,
		selected: z.boolean(),
		change: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('no-split') }).readonly(),
			z.strictObject({ kind: z.literal('split'), split: ClientSplitChangeSchema }).readonly(),
		]),
	})
	.readonly();

export const ClientPanelAreaSchema = z
	.strictObject({
		panelId: WorkbenchPanelIdSchema,
		unzoomed: ClientCellRectSchema.nullable(),
		terminal: ClientCellRectSchema.nullable(),
		displayed: ClientCellRectSchema.nullable(),
	})
	.readonly();
export const ClientSplitAreaSchema = z
	.strictObject({
		splitId: WorkbenchSplitIdSchema,
		firstSlotId: WorkbenchPanelSlotIdSchema,
		secondSlotId: WorkbenchPanelSlotIdSchema,
		orientation: z.enum(['horizontal', 'vertical']),
		ratio: ClientSplitRatioSchema,
		area: ClientCellRectSchema.nullable(),
		divider: ClientCellRectSchema.nullable(),
	})
	.readonly();
export const ClientPanelNeighborsSchema = z
	.strictObject({
		left: WorkbenchPanelIdSchema.nullable(),
		right: WorkbenchPanelIdSchema.nullable(),
		up: WorkbenchPanelIdSchema.nullable(),
		down: WorkbenchPanelIdSchema.nullable(),
	})
	.readonly();

type CellRect = z.infer<typeof ClientCellRectSchema>;
type PanelArea = z.infer<typeof ClientPanelAreaSchema>;
type Direction = z.infer<typeof ClientPanelDirectionSchema>;
function nonempty(rect: CellRect): boolean {
	return rect.width > 0 && rect.height > 0;
}
function contains(outer: CellRect, inner: CellRect): boolean {
	return (
		inner.x >= outer.x &&
		inner.y >= outer.y &&
		inner.x + inner.width <= outer.x + outer.width &&
		inner.y + inner.height <= outer.y + outer.height
	);
}
function neighbor(
	source: string,
	panels: readonly PanelArea[],
	direction: Direction,
): string | null {
	const from = panels.find((panel) => panel.panelId === source)?.unzoomed;
	if (from === null || from === undefined || !nonempty(from)) return null;
	let best: readonly [number, number, number] | null = null;
	let selected: string | null = null;
	for (const panel of panels) {
		const to = panel.unzoomed;
		if (panel.panelId === source || to === null || !nonempty(to)) continue;
		let distance: number;
		let start: number;
		let end: number;
		let otherStart: number;
		let otherEnd: number;
		switch (direction) {
			case 'left':
				distance = from.x - to.x - to.width;
				start = from.y;
				end = from.y + from.height;
				otherStart = to.y;
				otherEnd = to.y + to.height;
				break;
			case 'right':
				distance = to.x - from.x - from.width;
				start = from.y;
				end = from.y + from.height;
				otherStart = to.y;
				otherEnd = to.y + to.height;
				break;
			case 'up':
				distance = from.y - to.y - to.height;
				start = from.x;
				end = from.x + from.width;
				otherStart = to.x;
				otherEnd = to.x + to.width;
				break;
			case 'down':
				distance = to.y - from.y - from.height;
				start = from.x;
				end = from.x + from.width;
				otherStart = to.x;
				otherEnd = to.x + to.width;
				break;
		}
		const overlap = Math.min(end, otherEnd) - Math.max(start, otherStart);
		if (distance < 0 || overlap <= 0) continue;
		const center = Math.abs(start + end - otherStart - otherEnd);
		// Strict improvement retains the first canonical traversal entry for an exact tie.
		if (
			best === null ||
			distance < best[0] ||
			(distance === best[0] && (overlap > best[1] || (overlap === best[1] && center < best[2])))
		) {
			best = [distance, overlap, center];
			selected = panel.panelId;
		}
	}
	return selected;
}
export const ClientPanelLayoutSchema = z
	.strictObject({
		terminalContextId: WorkbenchTerminalContextIdSchema,
		tabId: WorkbenchTabIdSchema,
		sourcePanelId: WorkbenchPanelIdSchema,
		workbenchRevision: WorkbenchRevisionSchema,
		viewport: ClientCellRectSchema,
		area: ClientCellRectSchema,
		selected: z.boolean(),
		zoomed: z.boolean(),
		focusedPanelId: WorkbenchPanelIdSchema.nullable(),
		sourceAvailable: z.boolean(),
		neighbors: ClientPanelNeighborsSchema,
		panels: z
			.array(ClientPanelAreaSchema)
			.min(1)
			.max(WORKBENCH_MAXIMUM_PANEL_NODES_PER_CONTEXT)
			.readonly(),
		splits: z
			.array(ClientSplitAreaSchema)
			.max(WORKBENCH_MAXIMUM_PANEL_NODES_PER_CONTEXT)
			.readonly(),
	})
	.superRefine((layout, context) => {
		const reject = () =>
			context.addIssue({ code: 'custom', message: 'Invalid Client layout observation' });
		if (
			!nonempty(layout.viewport) ||
			!contains(layout.viewport, layout.area) ||
			layout.panels.length + layout.splits.length > WORKBENCH_MAXIMUM_PANEL_NODES_PER_CONTEXT ||
			layout.splits.length + 1 !== layout.panels.length
		) {
			reject();
			return;
		}
		const panelIds = new Set<string>();
		for (const panel of layout.panels) {
			if (
				panelIds.has(panel.panelId) ||
				[panel.unzoomed, panel.terminal, panel.displayed].some(
					(rect) => rect !== null && (!nonempty(rect) || !contains(layout.area, rect)),
				) ||
				(panel.terminal !== null &&
					(panel.unzoomed === null || !contains(panel.unzoomed, panel.terminal))) ||
				(layout.zoomed && panel.displayed !== null && layout.focusedPanelId !== panel.panelId)
			) {
				reject();
				return;
			}
			panelIds.add(panel.panelId);
		}
		const source = layout.panels.find((panel) => panel.panelId === layout.sourcePanelId);
		if (
			source === undefined ||
			layout.sourceAvailable !== (source.unzoomed !== null) ||
			(layout.focusedPanelId !== null && !panelIds.has(layout.focusedPanelId)) ||
			(layout.selected && layout.focusedPanelId === null)
		) {
			reject();
			return;
		}
		const directions: readonly Direction[] = ['left', 'right', 'up', 'down'];
		for (const direction of directions) {
			if (
				layout.neighbors[direction] !== neighbor(layout.sourcePanelId, layout.panels, direction)
			) {
				reject();
				return;
			}
		}
		const splitIds = new Set<string>();
		const slotIds = new Set<string>();
		for (const split of layout.splits) {
			if (
				splitIds.has(split.splitId) ||
				slotIds.has(split.firstSlotId) ||
				slotIds.has(split.secondSlotId) ||
				split.firstSlotId === split.secondSlotId ||
				(split.area !== null && (!nonempty(split.area) || !contains(layout.area, split.area))) ||
				(split.divider !== null &&
					(!nonempty(split.divider) ||
						split.area === null ||
						!contains(split.area, split.divider) ||
						(split.orientation === 'horizontal'
							? split.divider.width !== 1
							: split.divider.height !== 1)))
			) {
				reject();
				return;
			}
			splitIds.add(split.splitId);
			slotIds.add(split.firstSlotId);
			slotIds.add(split.secondSlotId);
		}
		if (
			new TextEncoder().encode(JSON.stringify(layout)).byteLength >
			WORKBENCH_MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES - 4096
		)
			reject();
	})
	.readonly();

export type ClientCellRect = z.infer<typeof ClientCellRectSchema>;
export type ClientPanelDirection = z.infer<typeof ClientPanelDirectionSchema>;
export type ClientPanelArea = z.infer<typeof ClientPanelAreaSchema>;
export type ClientSplitArea = z.infer<typeof ClientSplitAreaSchema>;
export type ClientPanelNeighbors = z.infer<typeof ClientPanelNeighborsSchema>;
export type ClientPanelLayout = z.infer<typeof ClientPanelLayoutSchema>;
