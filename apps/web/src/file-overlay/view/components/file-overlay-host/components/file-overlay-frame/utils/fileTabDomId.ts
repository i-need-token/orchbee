/** A tab's element id, by which the content panel names the active tab. */
export function fileTabDomId(idPrefix: string, index: number): string {
	return `${idPrefix}-tab-${index}`;
}
