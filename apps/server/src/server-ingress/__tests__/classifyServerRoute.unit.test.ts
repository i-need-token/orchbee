import { describe, expect, it } from 'vitest';

import { classifyServerRoute } from '../classifyServerRoute';

describe('classifyServerRoute', () => {
	it('keeps every reserved namespace out of the document fallback', () => {
		expect(
			classifyServerRoute({ acceptsHtml: true, method: 'GET', pathname: '/api/v1/status' }),
		).toBe('api-not-found');
		expect(
			classifyServerRoute({ acceptsHtml: true, method: 'GET', pathname: '/ws/v1/events' }),
		).toBe('websocket-upgrade-required');
		expect(
			classifyServerRoute({ acceptsHtml: true, method: 'GET', pathname: '/static/missing.js' }),
		).toBe('static-not-found');
	});

	it('matches namespace path segments rather than textual prefixes', () => {
		expect(classifyServerRoute({ acceptsHtml: true, method: 'GET', pathname: '/apiary' })).toBe(
			'document',
		);
		expect(
			classifyServerRoute({ acceptsHtml: true, method: 'GET', pathname: '/ws-settings' }),
		).toBe('document');
	});

	it('returns the document only for GET or HEAD requests that accept HTML', () => {
		expect(
			classifyServerRoute({ acceptsHtml: true, method: 'head', pathname: '/projects/alpha' }),
		).toBe('document');
		expect(
			classifyServerRoute({ acceptsHtml: false, method: 'GET', pathname: '/projects/alpha' }),
		).toBe('not-found');
		expect(
			classifyServerRoute({ acceptsHtml: true, method: 'POST', pathname: '/projects/alpha' }),
		).toBe('not-found');
	});
});
