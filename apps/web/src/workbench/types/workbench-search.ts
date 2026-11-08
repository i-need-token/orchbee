export const WORKBENCH_SEARCH_FILTERS = ['all', 'agents', 'actions', 'settings'] as const;

export type WorkbenchSearchFilter = (typeof WORKBENCH_SEARCH_FILTERS)[number];
