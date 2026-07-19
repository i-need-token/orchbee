import { afterEach, describe, expect, it, vi } from 'vitest';
import { createBrowserTerminalHost } from '../createBrowserTerminalHost';

afterEach(() => vi.unstubAllGlobals());

describe('createBrowserTerminalHost', () => {
	it('reads and writes plain text through the browser clipboard', async () => {
		const readText = vi.fn<() => Promise<string>>(async () => 'clipboard text');
		const writeText = vi.fn<(text: string) => Promise<void>>(async () => {});
		vi.stubGlobal('navigator', { clipboard: { readText, writeText } });
		const host = createBrowserTerminalHost();

		await expect(host.clipboard?.read({ target: 'standard', source: 'paste' })).resolves.toEqual({
			text: 'clipboard text',
		});
		await host.clipboard?.write({
			target: 'standard',
			source: 'copy',
			payload: { text: 'selected text' },
		});

		expect(readText).toHaveBeenCalledOnce();
		expect(writeText).toHaveBeenCalledWith('selected text');
		expect(host.capabilities).toMatchObject({ clipboardRead: true, clipboardWrite: true });
	});

	it('delegates paste confirmation and URL opening to browser capabilities', async () => {
		const confirm = vi.fn<(message?: string) => boolean>(() => true);
		const open = vi.fn<
			(url?: string | URL, target?: string, features?: string) => WindowProxy | null
		>(() => ({ closed: false }) as WindowProxy);
		vi.stubGlobal('window', { confirm, open });
		const host = createBrowserTerminalHost();

		await expect(
			host.confirmPaste?.({
				text: 'first\nsecond',
				sanitized: 'first\nsecond',
				bracketed: false,
				multiline: true,
				embeddedBracketTerminator: false,
				replacedControlBytes: 0,
			}),
		).resolves.toBe(true);
		await host.openTarget?.({
			source: 'link',
			target: { kind: 'url', url: 'https://example.com' },
		});

		expect(confirm).toHaveBeenCalledWith('Paste 2 lines into the terminal?');
		expect(open).toHaveBeenCalledWith('https://example.com', '_blank', 'noopener,noreferrer');
	});

	it('rejects unsupported local file targets', async () => {
		vi.stubGlobal('window', {
			open: vi.fn<(url?: string | URL, target?: string, features?: string) => WindowProxy | null>(
				() => null,
			),
		});
		const host = createBrowserTerminalHost();
		await expect(
			host.openTarget?.({ source: 'api', target: { kind: 'file', path: '/tmp/file.txt' } }),
		).rejects.toThrow('cannot open local file targets');
	});

	it('does not advertise a clipboard adapter when the browser has none', () => {
		vi.stubGlobal('navigator', {});
		const host = createBrowserTerminalHost();
		expect(host.clipboard).toBeUndefined();
		expect(host.capabilities).toMatchObject({ clipboardRead: false, clipboardWrite: false });
	});
});
