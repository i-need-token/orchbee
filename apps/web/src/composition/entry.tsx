import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
import '@orchbee/workbench/styles.css';

import { resolveShortcutPlatform } from './resolveShortcutPlatform';
import { startWebApplication } from './startWebApplication';

startWebApplication(
	document.querySelector('#root'),
	document.documentElement,
	navigator.languages,
	resolveShortcutPlatform(navigator.platform),
	window.localStorage,
);
