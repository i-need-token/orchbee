import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { SystemTestFixture } from './SystemTestFixture';
import '../../reference/tab-strip.css';
import './fixture.css';

const rootElement = document.getElementById('root');
if (!rootElement) throw new Error('Expected the system-test fixture root element.');

createRoot(rootElement).render(
	<StrictMode>
		<SystemTestFixture />
	</StrictMode>,
);
