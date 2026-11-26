import {
	activateFloatTab,
	closeFloatTabs,
	collapseFloat,
	EMPTY_FLOAT,
	expandFloat,
	type FloatState,
	type FloatTarget,
	keepFloatTab,
	openFloatTab,
} from './float-tabs';

/** Every Workspace's float window, so switching Workspaces brings each one's files back. */
export type FloatWindows = ReadonlyMap<string, FloatState>;

export type FloatEvent =
	| {
			readonly kind: 'open';
			readonly workspaceId: string;
			readonly target: FloatTarget;
			readonly preview: boolean;
	  }
	| { readonly kind: 'activate'; readonly workspaceId: string; readonly id: string }
	| { readonly kind: 'keep'; readonly workspaceId: string; readonly id: string }
	| { readonly kind: 'close'; readonly workspaceId: string; readonly ids: ReadonlySet<string> }
	| { readonly kind: 'collapse'; readonly workspaceId: string }
	| { readonly kind: 'expand'; readonly workspaceId: string };

export function floatOf(windows: FloatWindows, workspaceId: string | null): FloatState {
	return (workspaceId === null ? undefined : windows.get(workspaceId)) ?? EMPTY_FLOAT;
}

export function reduceFloatWindows(windows: FloatWindows, event: FloatEvent): FloatWindows {
	const current = floatOf(windows, event.workspaceId);
	let next: FloatState;
	switch (event.kind) {
		case 'open':
			next = openFloatTab(current, event.target, { preview: event.preview });
			break;
		case 'activate':
			next = activateFloatTab(current, event.id);
			break;
		case 'keep':
			next = keepFloatTab(current, event.id);
			break;
		case 'close':
			next = closeFloatTabs(current, event.ids);
			break;
		case 'collapse':
			next = collapseFloat(current);
			break;
		case 'expand':
			next = expandFloat(current);
			break;
	}
	if (next === current) return windows;
	const updated = new Map(windows);
	if (next.tabs.length === 0) updated.delete(event.workspaceId);
	else updated.set(event.workspaceId, next);
	return updated;
}
