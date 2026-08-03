import { describe, expect, it, vi } from 'vitest';

import { createBrowserTerminalPlatformEffects } from '../createBrowserTerminalPlatformEffects';

describe('createBrowserTerminalPlatformEffects', () => {
	it('writes Host-exported text through the Browser clipboard capability', async () => {
		const writeClipboardText = vi.fn<(text: string) => Promise<void>>().mockResolvedValue();
		const effects = createBrowserTerminalPlatformEffects({
			openExternal: vi.fn<(target: string) => void>(),
			writeClipboardText,
		});

		expect(await effects.writeClipboardText?.('selected')).toEqual({ kind: 'completed' });
		expect(writeClipboardText).toHaveBeenCalledWith('selected');
	});

	it('returns typed failures for unavailable clipboard and blocked links', async () => {
		const openExternal = vi.fn<(target: string) => void>();
		const effects = createBrowserTerminalPlatformEffects({ openExternal });

		expect(await effects.writeClipboardText?.('selected')).toMatchObject({
			code: 'browser-clipboard-unavailable',
			kind: 'failed',
		});
		expect(effects.openExternal?.('file:///tmp/secret')).toMatchObject({
			code: 'link-policy-blocked',
			kind: 'failed',
		});
		expect(openExternal).not.toHaveBeenCalled();
	});

	it('opens only canonical allowlisted Browser targets', () => {
		const openExternal = vi.fn<(target: string) => void>();
		const effects = createBrowserTerminalPlatformEffects({ openExternal });

		expect(effects.openExternal?.('https://orchbee.dev/终端')).toEqual({ kind: 'completed' });
		expect(openExternal).toHaveBeenCalledWith('https://orchbee.dev/%E7%BB%88%E7%AB%AF');
	});
});
