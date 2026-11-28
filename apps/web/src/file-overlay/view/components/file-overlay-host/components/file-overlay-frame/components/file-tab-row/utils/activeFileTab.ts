/** The row's active tab, which takes the keys whenever the row hands them to a tab. */
export function activeFileTab(row: HTMLElement): HTMLElement | null {
	return row.querySelector<HTMLElement>('[role="tab"][data-active]');
}
