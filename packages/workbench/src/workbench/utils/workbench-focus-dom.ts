import type { WorkbenchFocusRegion } from '../model/resolveWorkbenchFocusTarget';

const WORKBENCH_EXCLUSIVE_FOCUS_SURFACE_SELECTOR =
	'[data-workbench-keyboard-capture="true"], [role="dialog"], [role="menu"], [data-slot="dialog-content"], [data-slot="alert-dialog-content"], [data-slot="context-menu-content"], [data-slot="dropdown-menu-content"]';

export function resolveWorkbenchFocusRegionFromTarget(
	target: EventTarget | null,
): WorkbenchFocusRegion | null {
	if (!(target instanceof Element)) return null;
	const region = target.closest<HTMLElement>('[data-workbench-focus-region]')?.dataset[
		'workbenchFocusRegion'
	];
	return region === 'terminal' || region === 'tab-strip' || region === 'sidebar' ? region : null;
}

export function isWorkbenchExclusiveFocusTarget(target: EventTarget | null): boolean {
	return (
		target instanceof Element && target.closest(WORKBENCH_EXCLUSIVE_FOCUS_SURFACE_SELECTOR) !== null
	);
}

export function hasWorkbenchExclusiveFocusSurface(root: ParentNode): boolean {
	return root.querySelector(WORKBENCH_EXCLUSIVE_FOCUS_SURFACE_SELECTOR) !== null;
}
