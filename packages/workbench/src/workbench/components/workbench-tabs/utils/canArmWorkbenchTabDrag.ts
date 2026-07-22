interface WorkbenchTabDragPointerDown {
	readonly button: number;
	readonly currentTarget: { readonly contains: (node: Node | null) => boolean };
	readonly isPrimary: boolean;
	readonly target: EventTarget | null;
}

export function canArmWorkbenchTabDrag(event: WorkbenchTabDragPointerDown): boolean {
	return (
		event.button === 0 &&
		event.isPrimary &&
		event.target instanceof Node &&
		event.currentTarget.contains(event.target)
	);
}
