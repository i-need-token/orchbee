import { describe, expect, it, vi } from 'vitest';

import { createBrowserTerminalPlatformEffects } from '../createBrowserTerminalPlatformEffects';

describe('createBrowserTerminalPlatformEffects', () => {
	it('writes Host-exported text through the Browser clipboard capability', async () => {
		const writeClipboardText = vi.fn<(text: string) => Promise<void>>().mockResolvedValue();
		const effects = createBrowserTerminalPlatformEffects({
			getClipboardTextWriter: () => writeClipboardText,
			hasTransientUserActivation: () => true,
			isSecureContext: () => true,
			openExternal: vi.fn<(target: string) => void>(),
		});

		expect(effects.isClipboardWriteAvailable?.()).toBe(true);
		expect(await effects.writeClipboardText?.('selected')).toEqual({ kind: 'completed' });
		expect(writeClipboardText).toHaveBeenCalledWith('selected');
	});

	it('rechecks secure context and transient user activation for every clipboard operation', async () => {
		let secureContext = false;
		let transientUserActivation = true;
		const writeClipboardText = vi.fn<(text: string) => Promise<void>>().mockResolvedValue();
		let clipboardTextWriter: typeof writeClipboardText | undefined = writeClipboardText;
		const effects = createBrowserTerminalPlatformEffects({
			getClipboardTextWriter: () => clipboardTextWriter,
			hasTransientUserActivation: () => transientUserActivation,
			isSecureContext: () => secureContext,
			openExternal: vi.fn<(target: string) => void>(),
		});

		expect(effects.isClipboardWriteAvailable?.()).toBe(false);
		expect(await effects.writeClipboardText?.('insecure')).toMatchObject({
			code: 'browser-clipboard-insecure-context',
			kind: 'failed',
		});
		secureContext = true;
		transientUserActivation = false;
		expect(effects.isClipboardWriteAvailable?.()).toBe(false);
		expect(await effects.writeClipboardText?.('not activated')).toMatchObject({
			code: 'browser-clipboard-user-activation-required',
			kind: 'failed',
		});
		transientUserActivation = true;
		clipboardTextWriter = undefined;
		expect(effects.isClipboardWriteAvailable?.()).toBe(false);
		expect(await effects.writeClipboardText?.('missing API')).toMatchObject({
			code: 'browser-clipboard-unavailable',
			kind: 'failed',
		});
		clipboardTextWriter = writeClipboardText;
		expect(effects.isClipboardWriteAvailable?.()).toBe(true);
		expect(await effects.writeClipboardText?.('activated')).toEqual({ kind: 'completed' });
		transientUserActivation = false;
		expect(await effects.writeClipboardText?.('expired activation')).toMatchObject({
			code: 'browser-clipboard-user-activation-required',
			kind: 'failed',
		});

		expect(writeClipboardText).toHaveBeenCalledExactlyOnceWith('activated');
		expect('readClipboardText' in effects).toBe(false);
	});

	it('fails closed after one rejected write without retry or fallback', async () => {
		const writeClipboardText = vi
			.fn<(text: string) => Promise<void>>()
			.mockRejectedValue(new DOMException('Denied', 'NotAllowedError'));
		const effects = createBrowserTerminalPlatformEffects({
			getClipboardTextWriter: () => writeClipboardText,
			hasTransientUserActivation: () => true,
			isSecureContext: () => true,
			openExternal: vi.fn<(target: string) => void>(),
		});

		expect(await effects.writeClipboardText?.('selected')).toEqual({
			code: 'browser-clipboard-write-failed',
			kind: 'failed',
			retryable: false,
		});
		expect(writeClipboardText).toHaveBeenCalledExactlyOnceWith('selected');
	});

	it('does not resolve a clipboard writer in an insecure context and bounds probe failures', async () => {
		const getClipboardTextWriter = vi.fn<() => ((text: string) => Promise<void>) | undefined>();
		const hasTransientUserActivation = vi.fn<() => boolean>(() => true);
		const insecureEffects = createBrowserTerminalPlatformEffects({
			getClipboardTextWriter,
			hasTransientUserActivation,
			isSecureContext: () => false,
			openExternal: vi.fn<(target: string) => void>(),
		});

		expect(await insecureEffects.writeClipboardText?.('insecure')).toMatchObject({
			code: 'browser-clipboard-insecure-context',
			kind: 'failed',
		});
		expect(getClipboardTextWriter).not.toHaveBeenCalled();
		expect(hasTransientUserActivation).not.toHaveBeenCalled();

		const failedProbeEffects = createBrowserTerminalPlatformEffects({
			getClipboardTextWriter,
			hasTransientUserActivation,
			isSecureContext: () => {
				throw new Error('probe failed');
			},
			openExternal: vi.fn<(target: string) => void>(),
		});
		expect(failedProbeEffects.isClipboardWriteAvailable?.()).toBe(false);
		expect(await failedProbeEffects.writeClipboardText?.('failed probe')).toMatchObject({
			code: 'browser-clipboard-capability-check-failed',
			kind: 'failed',
		});
		expect(getClipboardTextWriter).not.toHaveBeenCalled();
	});

	it('returns typed failures for unavailable clipboard and every unapproved link scheme', async () => {
		const openExternal = vi.fn<(target: string) => void>();
		const effects = createBrowserTerminalPlatformEffects({
			getClipboardTextWriter: () => undefined,
			hasTransientUserActivation: () => true,
			isSecureContext: () => true,
			openExternal,
		});

		expect(await effects.writeClipboardText?.('selected')).toMatchObject({
			code: 'browser-clipboard-unavailable',
			kind: 'failed',
		});
		for (const target of [
			'file:///tmp/secret',
			'ftp://orchbee.dev',
			'ssh://orchbee.dev',
			'tel:+15551234567',
			'javascript:alert(1)',
		]) {
			expect(effects.openExternal?.(target)).toMatchObject({
				code: 'link-policy-blocked',
				kind: 'failed',
			});
		}
		expect(openExternal).not.toHaveBeenCalled();
	});

	it('opens only canonical allowlisted Browser targets', () => {
		const openExternal = vi.fn<(target: string) => void>();
		const effects = createBrowserTerminalPlatformEffects({
			getClipboardTextWriter: () => undefined,
			hasTransientUserActivation: () => true,
			isSecureContext: () => true,
			openExternal,
		});

		for (const target of [
			'http://orchbee.dev/path',
			'https://orchbee.dev/终端',
			'mailto:terminal@orchbee.dev',
		]) {
			expect(effects.openExternal?.(target)).toEqual({ kind: 'completed' });
		}
		expect(openExternal.mock.calls.map(([target]) => target)).toEqual([
			'http://orchbee.dev/path',
			'https://orchbee.dev/%E7%BB%88%E7%AB%AF',
			'mailto:terminal@orchbee.dev',
		]);
	});
});
