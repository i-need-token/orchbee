import type { WorkbenchFocusRegion } from '../model/resolveWorkbenchFocusTarget';

const WORKBENCH_EXCLUSIVE_FOCUS_SURFACE_SELECTOR =
	'[data-workbench-keyboard-capture="true"], [role="dialog"], [role="menu"], [data-slot="dialog-content"], [data-slot="alert-dialog-content"], [data-slot="context-menu-content"], [data-slot="dropdown-menu-content"]';

/** The surface whose rows run focus Actions, which act once it has closed and given focus back. */
const WORKBENCH_COMMAND_SURFACE_SELECTOR = '[data-workbench-command-surface="true"]';

export function resolveWorkbenchFocusRegionFromTarget(
	target: EventTarget | null,
): WorkbenchFocusRegion | null {
	if (!(target instanceof Element)) return null;
	const region = target.closest<HTMLElement>('[data-workbench-focus-region]')?.dataset[
		'workbenchFocusRegion'
	];
	return region === 'terminal' || region === 'tab-strip' || region === 'sidebar' ? region : null;
}

/** With `exceptCommandSurface`, the command surface and what it holds do not count. */
export function isWorkbenchExclusiveFocusTarget(
	target: EventTarget | null,
	exceptCommandSurface = false,
): boolean {
	if (!(target instanceof Element)) return false;
	const surface = target.closest(WORKBENCH_EXCLUSIVE_FOCUS_SURFACE_SELECTOR);
	return surface !== null && !(exceptCommandSurface && isInCommandSurface(surface));
}

/** With `exceptCommandSurface`, the command surface and what it holds do not count. */
export function hasWorkbenchExclusiveFocusSurface(
	root: ParentNode,
	exceptCommandSurface = false,
): boolean {
	if (!exceptCommandSurface) {
		return root.querySelector(WORKBENCH_EXCLUSIVE_FOCUS_SURFACE_SELECTOR) !== null;
	}
	return Array.from(root.querySelectorAll(WORKBENCH_EXCLUSIVE_FOCUS_SURFACE_SELECTOR)).some(
		(surface) => !isInCommandSurface(surface),
	);
}

export function hasWorkbenchCommandSurface(root: ParentNode): boolean {
	return root.querySelector(WORKBENCH_COMMAND_SURFACE_SELECTOR) !== null;
}

function isInCommandSurface(element: Element): boolean {
	return element.closest(WORKBENCH_COMMAND_SURFACE_SELECTOR) !== null;
}
