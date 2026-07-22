import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '@orchbee/ui/styles.css';
import { SystemTestFixture } from './SystemTestFixture';
import '../../src/styles';
import './fixture.css';

const rootElement = document.getElementById('root');
if (!rootElement) throw new Error('Expected the system-test fixture root element.');

createRoot(rootElement).render(
	<StrictMode>
		<SystemTestFixture />
	</StrictMode>,
);
