import type { WorkbenchTab } from '@orchbee/workbench';

export const DEVELOPMENT_WORKBENCH_TABS = [
	{ icon: 'terminal', id: 'orchbee', label: '~/github/orchbee' },
	{ icon: 'terminal', id: 'workbench-tabs', label: 'workbench/tabs' },
	{ icon: 'terminal', id: 'ui-tabs', label: 'ui/tabs' },
	{ icon: 'terminal', id: 'localization', label: 'i18n/workbench' },
	{ icon: 'terminal', id: 'verification', label: 'tests/verification' },
	{ icon: 'terminal', id: 'frontend-docs', label: 'docs/frontend' },
] as const satisfies readonly WorkbenchTab[];
