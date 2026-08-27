import { describe, expect, it, vi } from 'vitest';

import {
	BrowserAuthenticationClientError,
	type BrowserAuthenticationClientPort,
} from '../browser-authentication-client';
import {
	BrowserAuthenticationController,
	resolveBrowserAuthenticationReturnTarget,
} from '../browser-authentication-controller';

describe('BrowserAuthenticationController', () => {
	it('validates input and resumes only the same-origin relative return target after success', async () => {
		const authenticate = vi.fn<BrowserAuthenticationClientPort['authenticate']>(
			async () => undefined,
		);
		const navigate = vi.fn<(target: string) => void>();
		const controller = new BrowserAuthenticationController({
			client: { authenticate },
			navigate,
			returnTarget: '/projects/alpha?panel=terminal',
		});

		controller.submit();
		expect(controller.getSnapshot().error).toBe('required');
		controller.setPersonalAccessToken('p'.repeat(43));
		controller.submit();
		await vi.waitFor(() => expect(navigate).toHaveBeenCalledWith('/projects/alpha?panel=terminal'));

		expect(authenticate).toHaveBeenCalledWith('p'.repeat(43));
		expect(controller.getSnapshot().personalAccessToken).toBe('');
		controller.dispose();
	});

	it('keeps a bounded authentication failure available for localized rendering', async () => {
		const controller = new BrowserAuthenticationController({
			client: {
				authenticate: vi.fn<BrowserAuthenticationClientPort['authenticate']>(async () => {
					throw new BrowserAuthenticationClientError('invalid-personal-access-token');
				}),
			},
			navigate: vi.fn<(target: string) => void>(),
			returnTarget: '/',
		});
		controller.setPersonalAccessToken('p'.repeat(43));

		controller.submit();
		await vi.waitFor(() => expect(controller.getSnapshot().pending).toBe(false));

		expect(controller.getSnapshot().error).toBe('invalid-personal-access-token');
		controller.dispose();
	});
});

describe('resolveBrowserAuthenticationReturnTarget', () => {
	it('accepts one same-origin relative path and rejects external, duplicate, and login targets', () => {
		expect(
			resolveBrowserAuthenticationReturnTarget(
				'?returnTo=%2F%3Forchbee-runtime%3Dgateway%26orchbee-gateway-port%3D43111',
			),
		).toBe('/?orchbee-runtime=gateway&orchbee-gateway-port=43111');
		expect(resolveBrowserAuthenticationReturnTarget('?returnTo=https%3A%2F%2Fevil.example')).toBe(
			'/',
		);
		expect(resolveBrowserAuthenticationReturnTarget('?returnTo=%2F%2Fevil.example')).toBe('/');
		expect(resolveBrowserAuthenticationReturnTarget('?returnTo=%2Flogin')).toBe('/');
		expect(resolveBrowserAuthenticationReturnTarget('?returnTo=%2Fa&returnTo=%2Fb')).toBe('/');
	});
});
