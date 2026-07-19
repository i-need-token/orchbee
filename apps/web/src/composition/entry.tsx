import '@orchbee/ui/styles.css';
import '@orchbee/workbench/styles.css';

import { startWebApplication } from './startWebApplication';

const rootElement = document.querySelector('#root');
if (rootElement === null) throw new Error('The Orchbee web root element is missing.');

startWebApplication(rootElement, document.documentElement, navigator.languages);
