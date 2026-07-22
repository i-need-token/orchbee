const WORKBENCH_TAB_CONTROL_SELECTOR = [
	'[data-workbench-tab-item-id] [role="tab"]',
	'[data-workbench-tab-item-id] [data-workbench-tab-action]',
	'[data-workbench-tab-group-id]',
	'[data-slot="workbench-tab-add"]',
].join(',');

export function activateWorkbenchContextMenuOutsideTarget(event: Event): void {
	if (!(event instanceof MouseEvent)) return;
	if (event.button !== 0) return;
	const eventTarget = event.target;
	if (!(eventTarget instanceof Node)) return;
	const ownerDocument = eventTarget.ownerDocument;
	if (ownerDocument === null) return;

	const target = ownerDocument
		.elementsFromPoint(event.clientX, event.clientY)
		.map((element) => element.closest<HTMLElement>(WORKBENCH_TAB_CONTROL_SELECTOR))
		.find((element) => element !== null);
	target?.click();
}
