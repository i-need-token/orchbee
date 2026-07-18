import '@orchbee/ui/styles.css';
import '@orchbee/workbench/styles.css';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';

import { createInMemorySessionService } from '../sessions';
import { WebApplication } from './web-application';

const rootElement = document.querySelector('#root');
if (rootElement === null) throw new Error('The Orchbee web root element is missing.');

const sessions = createInMemorySessionService();
createRoot(rootElement).render(createElement(WebApplication, { sessions }));
