import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { squeezeFileTabs, type FileTabSlot } from '../squeezeFileTabs';

const GAP = 4;

/** Lays tabs of these widths out along a row from 0, with the gap between them. */
function layOut(widths: readonly number[]): FileTabSlot[] {
	let start = 0;
	return widths.map((size) => {
		const slot = { size, start };
		start += size + GAP;
		return slot;
	});
}

function at<Item>(items: readonly Item[], place: number): Item {
	const item = items[place];
	if (item === undefined) throw new Error(`Nothing at ${place}.`);
	return item;
}

/** A row of tabs, the tab a drag holds, and where the pointer puts that tab's start. */
const drag = fc
	.array(fc.integer({ max: 240, min: 24 }), { maxLength: 12, minLength: 2 })
	.chain((widths) =>
		fc.record({
			held: fc.nat({ max: widths.length - 1 }),
			start: fc.integer({ max: 3200, min: -400 }),
			widths: fc.constant(widths),
		}),
	);

describe('squeezeFileTabs', () => {
	it('slides every other tab to the slot the shown move gives it, and keeps the held one in the row', () => {
		fc.assert(
			fc.property(drag, ({ held, start, widths }) => {
				const slots = layOut(widths);
				const { index, offsets } = squeezeFileTabs(slots, held, start, GAP);
				const order = widths.map((_width, place) => place).filter((place) => place !== held);
				order.splice(index, 0, held);
				const moved = layOut(order.map((place) => at(widths, place)));
				for (const [place, tab] of order.entries()) {
					if (tab === held) continue;
					// Released, the row stands where the drag showed it, so nothing jumps.
					expect(at(slots, tab).start + at(offsets, tab)).toBe(at(moved, place).start);
				}
				const shown = at(slots, held).start + at(offsets, held);
				const last = at(slots, slots.length - 1);
				expect(shown).toBeGreaterThanOrEqual(0);
				expect(shown + at(widths, held)).toBeLessThanOrEqual(last.start + last.size);
			}),
		);
	});

	it('moves the held tab no nearer the start as the pointer goes on, and puts it back where it began', () => {
		fc.assert(
			fc.property(drag, fc.integer({ max: 400, min: 0 }), ({ held, start, widths }, step) => {
				const slots = layOut(widths);
				const here = squeezeFileTabs(slots, held, start, GAP);
				expect(squeezeFileTabs(slots, held, start + step, GAP).index).toBeGreaterThanOrEqual(
					here.index,
				);
				expect(squeezeFileTabs(slots, held, at(slots, held).start, GAP)).toEqual({
					index: held,
					offsets: widths.map(() => 0),
				});
			}),
		);
	});
});
