/** Where a tab of the row sits along it, measured from the start of the row's list. */
export interface FileTabSlot {
	readonly size: number;
	readonly start: number;
}

/** What a drag shows: the place the held tab takes once let go, and how far each tab moves. */
export interface FileTabSqueeze {
	/** The held tab's place in the row once it is let go. */
	readonly index: number;
	/** How far each tab, the held one included, moves along the row from its slot. */
	readonly offsets: readonly number[];
}

/**
 * The squeeze motion of a drag along the row. The held tab follows `start`, kept within the row, and
 * once its leading edge crosses another tab's midpoint, that tab and every tab between them slide
 * into the place it left; crossing back slides them home.
 */
export function squeezeFileTabs(
	slots: readonly FileTabSlot[],
	held: number,
	start: number,
	gap: number,
): FileTabSqueeze {
	const heldSlot = slots[held];
	const first = slots[0];
	const last = slots.at(-1);
	if (heldSlot === undefined || first === undefined || last === undefined) {
		return { index: held, offsets: slots.map(() => 0) };
	}
	const heldStart = Math.min(Math.max(start, first.start), last.start + last.size - heldSlot.size);
	const heldEnd = heldStart + heldSlot.size;
	const shift = heldSlot.size + gap;
	let index = held;
	const offsets = slots.map((slot, place) => {
		if (place === held) return heldStart - heldSlot.start;
		const middle = slot.start + slot.size / 2;
		if (place > held && heldEnd > middle) {
			index += 1;
			return -shift;
		}
		if (place < held && heldStart < middle) {
			index -= 1;
			return shift;
		}
		return 0;
	});
	return { index, offsets };
}
