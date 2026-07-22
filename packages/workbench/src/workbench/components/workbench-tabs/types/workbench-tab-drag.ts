import type { WorkbenchTabsState } from '../../../../workbench-tabs';

export interface WorkbenchTabStripItemGeometry {
	readonly id: string;
	readonly kind: 'group' | 'tab';
	readonly left: number;
	readonly width: number;
}

interface WorkbenchTabDragSharedState {
	readonly grabOffset: number;
	readonly layout: readonly WorkbenchTabStripItemGeometry[];
	readonly pointerId: number;
	readonly pointerX: number;
	readonly pointerY: number;
	readonly sourceLeft: number;
	readonly sourcePointerX: number;
	readonly sourcePointerY: number;
	readonly sourceState: WorkbenchTabsState;
}

export type WorkbenchTabDragState =
	| { readonly kind: 'idle' }
	| (WorkbenchTabDragSharedState & {
			readonly draggedTabId: string;
			readonly draggedWidth: number;
			readonly kind: 'armed-tab';
	  })
	| (WorkbenchTabDragSharedState & {
			readonly draggedGroupId: string;
			readonly draggedWidth: number;
			readonly kind: 'armed-group';
	  })
	| (WorkbenchTabDragSharedState & {
			readonly draggedTabId: string;
			readonly draggedWidth: number;
			readonly kind: 'dragging-tab';
			readonly lastAcceptedPointerX: number;
			readonly previewState: WorkbenchTabsState;
	  })
	| (WorkbenchTabDragSharedState & {
			readonly draggedGroupId: string;
			readonly draggedWidth: number;
			readonly kind: 'dragging-group';
			readonly lastAcceptedPointerX: number;
			readonly previewState: WorkbenchTabsState;
	  })
	| {
			readonly kind: 'settling';
			readonly previewState: WorkbenchTabsState;
	  };

export type WorkbenchTabDragEvent =
	| {
			readonly draggedTabId: string;
			readonly draggedWidth: number;
			readonly grabOffset: number;
			readonly kind: 'arm-tab';
			readonly layout: readonly WorkbenchTabStripItemGeometry[];
			readonly pointerId: number;
			readonly pointerX: number;
			readonly pointerY: number;
			readonly sourceLeft: number;
			readonly state: WorkbenchTabsState;
	  }
	| {
			readonly draggedGroupId: string;
			readonly draggedWidth: number;
			readonly grabOffset: number;
			readonly kind: 'arm-group';
			readonly layout: readonly WorkbenchTabStripItemGeometry[];
			readonly pointerId: number;
			readonly pointerX: number;
			readonly pointerY: number;
			readonly sourceLeft: number;
			readonly state: WorkbenchTabsState;
	  }
	| {
			readonly kind: 'move';
			readonly pointerId: number;
			readonly primaryButtonPressed: boolean;
			readonly pointerX: number;
			readonly pointerY: number;
	  }
	| { readonly kind: 'release'; readonly pointerId: number }
	| { readonly kind: 'cancel'; readonly pointerId?: number | undefined }
	| { readonly kind: 'acknowledge-commit' }
	| { readonly kind: 'stale-input' };

export type WorkbenchTabDragCommand =
	| { readonly kind: 'activate-tab'; readonly tabId: string }
	| { readonly groupId: string; readonly kind: 'activate-tab-group' }
	| {
			readonly groupId: string | undefined;
			readonly kind: 'commit-tab-move';
			readonly tabId: string;
			readonly targetIndex: number;
	  }
	| {
			readonly groupId: string;
			readonly kind: 'commit-tab-group-move';
			readonly targetIndex: number;
	  };

export interface WorkbenchTabDragTransition {
	readonly commands: readonly WorkbenchTabDragCommand[];
	readonly state: WorkbenchTabDragState;
}
