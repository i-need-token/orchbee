export type WorkbenchTabColor =
	| 'blue'
	| 'cyan'
	| 'default'
	| 'gray'
	| 'green'
	| 'orange'
	| 'pink'
	| 'purple'
	| 'red'
	| 'yellow';

export type WorkbenchTabGroupColor = Exclude<WorkbenchTabColor, 'default'>;

export type WorkbenchTabIcon = 'terminal';

export interface WorkbenchTabGroup {
	readonly color: WorkbenchTabGroupColor;
	readonly id: string;
	readonly isCollapsed?: boolean | undefined;
	readonly label: string;
}

export interface WorkbenchTabGroupUpdate {
	readonly color?: WorkbenchTabGroupColor | undefined;
	readonly label?: string | undefined;
}

export interface WorkbenchTab {
	readonly color?: WorkbenchTabColor | undefined;
	readonly groupId?: string | undefined;
	readonly icon?: WorkbenchTabIcon | undefined;
	readonly id: string;
	readonly isPinned?: boolean | undefined;
	readonly label: string;
}

export type WorkbenchTabInsertInput = Omit<WorkbenchTab, 'groupId'>;

export interface WorkbenchTabsState {
	readonly activeTabId: string | undefined;
	readonly tabGroups: readonly WorkbenchTabGroup[];
	readonly tabs: readonly WorkbenchTab[];
}

export interface WorkbenchTabsStateInput {
	readonly activeTabId?: string | undefined;
	readonly tabGroups?: readonly WorkbenchTabGroup[] | undefined;
	readonly tabs: readonly WorkbenchTab[];
}

export type WorkbenchTabsAction =
	| { readonly kind: 'activate-tab'; readonly tabId: string }
	| {
			readonly kind: 'assign-tab-group';
			readonly groupId: string | undefined;
			readonly tabId: string;
	  }
	| { readonly kind: 'close-tab-group'; readonly groupId: string }
	| { readonly kind: 'close-tabs'; readonly tabIds: readonly string[] }
	| {
			readonly group: WorkbenchTabGroup;
			readonly kind: 'create-tab-group';
			readonly tabIds: readonly string[];
	  }
	| { readonly groupId: string; readonly kind: 'move-tab-group'; readonly targetIndex: number }
	| {
			readonly groupId: string | undefined;
			readonly kind: 'move-tab';
			readonly tabId: string;
			readonly targetIndex: number;
	  }
	| {
			readonly groupId: string | undefined;
			readonly kind: 'insert-tab';
			readonly tab: WorkbenchTabInsertInput;
	  }
	| { readonly color: WorkbenchTabColor; readonly kind: 'set-tab-color'; readonly tabId: string }
	| {
			readonly groupId: string;
			readonly isCollapsed: boolean;
			readonly kind: 'set-tab-group-collapsed';
	  }
	| { readonly isPinned: boolean; readonly kind: 'set-tab-pinned'; readonly tabId: string }
	| { readonly kind: 'rename-tab'; readonly label: string; readonly tabId: string }
	| { readonly groupId: string; readonly kind: 'ungroup-tab-group' }
	| {
			readonly groupId: string;
			readonly kind: 'update-tab-group';
			readonly update: WorkbenchTabGroupUpdate;
	  };
