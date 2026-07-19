import '@orchbee/ui/styles.css';
import '@orchbee/workbench/styles.css';

import type { WorkbenchTab } from '@orchbee/workbench';

import { DEVELOPMENT_WORKBENCH_TABS } from './development-workbench-tabs';
import { resolveShortcutPlatform } from './resolveShortcutPlatform';
import { startWebApplication } from './startWebApplication';

const EMPTY_WORKBENCH_TABS: readonly WorkbenchTab[] = [];

const rootElement = document.querySelector('#root');
if (rootElement === null) throw new Error('The Orchbee web root element is missing.');

startWebApplication(
	rootElement,
	document.documentElement,
	navigator.languages,
	resolveShortcutPlatform(navigator.platform),
	window.localStorage,
	ORCHBEE_DEVELOPMENT ? DEVELOPMENT_WORKBENCH_TABS : EMPTY_WORKBENCH_TABS,
);
