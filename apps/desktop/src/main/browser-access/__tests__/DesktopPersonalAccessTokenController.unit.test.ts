import {
	PersonalAccessTokenSchema,
	PersonalAccessTokenVerifierSchema,
	type PersonalAccessTokenVerifier,
} from '@orchbee/remote-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopPersonalAccessTokenController,
	type DesktopPersonalAccessTokenStore,
} from '../DesktopPersonalAccessTokenController';

describe('DesktopPersonalAccessTokenController', () => {
	it('loads an absent verifier without creating a credential', async () => {
		const store = createStore(null);
		const controller = new DesktopPersonalAccessTokenController(store);

		await expect(controller.initialize()).resolves.toBe('missing');
		expect(controller.getVerifier()).toBeNull();
		expect(store.save).not.toHaveBeenCalled();
	});

	it('commits a verifier before revealing a newly issued Personal Access Token', async () => {
		const store = createStore(null);
		const token = PersonalAccessTokenSchema.parse('t'.repeat(43));
		const verifier = PersonalAccessTokenVerifierSchema.parse('v'.repeat(43));
		const controller = new DesktopPersonalAccessTokenController(store, {
			createToken: () => token,
			deriveVerifier: () => verifier,
		});
		await controller.initialize();

		await expect(controller.issue()).resolves.toBe(token);
		expect(store.save).toHaveBeenCalledWith(verifier);
		expect(controller.getVerifier()).toBe(verifier);
		expect(controller.getStatus()).toBe('configured');
	});

	it('preserves the prior verifier when persistence rejects rotation', async () => {
		const prior = PersonalAccessTokenVerifierSchema.parse('p'.repeat(43));
		const next = PersonalAccessTokenVerifierSchema.parse('n'.repeat(43));
		const store = createStore(prior);
		store.save.mockRejectedValueOnce(new Error('injected store failure'));
		const controller = new DesktopPersonalAccessTokenController(store, {
			createToken: () => PersonalAccessTokenSchema.parse('t'.repeat(43)),
			deriveVerifier: () => next,
		});
		await controller.initialize();

		await expect(controller.issue()).rejects.toThrow('injected store failure');
		expect(controller.getVerifier()).toBe(prior);
	});
});

function createStore(
	initial: PersonalAccessTokenVerifier | null,
): DesktopPersonalAccessTokenStore & {
	readonly load: ReturnType<typeof vi.fn>;
	readonly save: ReturnType<typeof vi.fn>;
} {
	return {
		load: vi.fn<DesktopPersonalAccessTokenStore['load']>(async () => initial),
		save: vi.fn<DesktopPersonalAccessTokenStore['save']>(async () => undefined),
	};
}
