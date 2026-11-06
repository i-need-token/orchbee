import {
	placesAfter,
	type SidebarDragHit,
	type SidebarDragItem,
} from '../model/sidebar-drag-destination';

/** Marks a drag entry: a Project heading, or a Workspace's whole item with its Agent rows. */
const SIDEBAR_DRAG_ENTRY_ATTRIBUTE = 'data-drag-entry';
/** The id of the Project or Workspace a drag entry stands for. */
const SIDEBAR_DRAG_ID_ATTRIBUTE = 'data-drag-id';
/** The End of Projects drop area after the list, present only while a Project is dragged. */
const SIDEBAR_DROP_AREA_ATTRIBUTE = 'data-sidebar-drop-area';

function readEntry(element: Element): SidebarDragItem | null {
	const kind = element.getAttribute(SIDEBAR_DRAG_ENTRY_ATTRIBUTE);
	const id = element.getAttribute(SIDEBAR_DRAG_ID_ATTRIBUTE);
	if (id === null) return null;
	if (kind === 'project') return { id, kind: 'project' };
	if (kind === 'workspace') return { id, kind: 'workspace' };
	return null;
}

/**
 * Reads what the pointer is over from the list as shown. A Project drag lands on Project headings
 * alone; a Workspace drag on headings and Workspace items. Recents and Agent rows are no entry.
 */
export function readSidebarDragHit(
	region: HTMLElement,
	held: SidebarDragItem,
	clientX: number,
	clientY: number,
	/** Where the press was, for a held entry the list does not show. */
	pressClientY: number,
): { readonly entries: readonly SidebarDragItem[]; readonly hit: SidebarDragHit } {
	const elements: HTMLElement[] = [];
	const entries: SidebarDragItem[] = [];
	for (const element of region.querySelectorAll<HTMLElement>(`[${SIDEBAR_DRAG_ENTRY_ATTRIBUTE}]`)) {
		const entry = readEntry(element);
		if (entry === null || (held.kind === 'project' && entry.kind !== 'project')) continue;
		elements.push(element);
		entries.push(entry);
	}
	const bounds = region.getBoundingClientRect();
	if (
		clientX < bounds.left ||
		clientX >= bounds.right ||
		clientY < bounds.top ||
		clientY >= bounds.bottom
	) {
		return { entries, hit: { kind: 'outside' } };
	}
	const dropArea = region.querySelector(`[${SIDEBAR_DROP_AREA_ATTRIBUTE}]`);
	// The drop area and the free space below it are one destination.
	if (dropArea !== null && clientY >= dropArea.getBoundingClientRect().top) {
		return { entries, hit: { kind: 'drop-area' } };
	}
	const index = elements.findIndex((element) => {
		const entryBounds = element.getBoundingClientRect();
		return clientY >= entryBounds.top && clientY < entryBounds.bottom;
	});
	const element = elements[index];
	if (element === undefined) return { entries, hit: { kind: 'gap' } };
	const entry = entries[index];
	const heldIndex = entries.findIndex(
		(candidate) => candidate.kind === held.kind && candidate.id === held.id,
	);
	const heldTop = elements[heldIndex]?.getBoundingClientRect().top ?? pressClientY;
	const entryBounds = element.getBoundingClientRect();
	const after = placesAfter(entryBounds, clientY, {
		belowHeld: entryBounds.top > heldTop,
		heading: entry?.kind === 'project',
	});
	return { entries, hit: { after, index, kind: 'entry' } };
}
