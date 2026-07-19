import type { WorkbenchTab } from '@orchbee/workbench';

export const DEVELOPMENT_WORKBENCH_TABS = [
	{ id: 'orchbee', label: '~/github/orchbee' },
	{ id: 'workbench-tabs', label: 'workbench/tabs' },
	{ id: 'ui-tabs', label: 'ui/tabs' },
	{ id: 'localization', label: 'i18n/workbench' },
	{ id: 'verification', label: 'tests/verification' },
	{ id: 'frontend-docs', label: 'docs/frontend' },
] as const satisfies readonly WorkbenchTab[];
