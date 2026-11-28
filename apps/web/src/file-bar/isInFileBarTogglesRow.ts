/**
 * Whether `target` lies in the toggles row the File Bar's column draws, which no overlay tier covers:
 * every tier starts below the 36px row.
 */
export function isInFileBarTogglesRow(target: EventTarget | null): boolean {
	return target instanceof Element && target.closest('[data-slot="file-bar-toggles-row"]') !== null;
}
