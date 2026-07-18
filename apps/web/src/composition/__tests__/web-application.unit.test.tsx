import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { createInMemorySessionService } from '../../sessions';
import { WebApplication } from '../web-application';

describe('WebApplication', () => {
	it('renders the application-owned session adapter through the Workbench', () => {
		const markup = renderToStaticMarkup(
			createElement(WebApplication, { sessions: createInMemorySessionService() }),
		);

		expect(markup).toContain('Local shell');
		expect(markup).toContain('Agent output');
	});
});
