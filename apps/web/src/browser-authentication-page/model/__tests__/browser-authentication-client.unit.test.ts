import { PersonalAccessTokenSchema } from '@orchbee/remote-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	BrowserAuthenticationClient,
	BrowserAuthenticationClientError,
} from '../browser-authentication-client';

const personalAccessToken = PersonalAccessTokenSchema.parse('p'.repeat(43));

describe('BrowserAuthenticationClient', () => {
	it('submits the Personal Access Token to the same-origin session endpoint', async () => {
		const fetchImplementation = vi.fn<typeof fetch>(
			async () => new Response(null, { status: 204 }),
		);
		const client = new BrowserAuthenticationClient(fetchImplementation);

		await client.authenticate(personalAccessToken);

		expect(fetchImplementation).toHaveBeenCalledWith(
			'/api/v1/auth/session',
			expect.objectContaining({
				body: JSON.stringify({ personalAccessToken }),
				credentials: 'same-origin',
				method: 'POST',
			}),
		);
	});

	it('preserves bounded server errors and hides malformed failures', async () => {
		const invalidClient = new BrowserAuthenticationClient(
			vi.fn<typeof fetch>(
				async () =>
					new Response(JSON.stringify({ error: { code: 'invalid-personal-access-token' } }), {
						status: 401,
					}),
			),
		);
		await expect(invalidClient.authenticate(personalAccessToken)).rejects.toEqual(
			expect.objectContaining({ code: 'invalid-personal-access-token' }),
		);

		const malformedClient = new BrowserAuthenticationClient(
			vi.fn<typeof fetch>(async () => new Response('not-json', { status: 500 })),
		);
		await expect(malformedClient.authenticate(personalAccessToken)).rejects.toEqual(
			new BrowserAuthenticationClientError('unavailable'),
		);
	});
});
