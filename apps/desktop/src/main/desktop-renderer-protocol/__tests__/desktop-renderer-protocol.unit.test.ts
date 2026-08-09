import path from 'node:path';
import { pathToFileURL } from 'node:url';

import type { Protocol } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	installDesktopRendererScheme,
	registerDesktopRendererScheme,
} from '../desktop-renderer-protocol';

type DesktopRendererProtocolHandler = Parameters<Protocol['handle']>[1];

async function installHandler(
	rendererRoot: string,
	fetchFile: (fileUrl: string) => Promise<Response>,
	fileProbe: (filePath: string) => Promise<boolean>,
): Promise<DesktopRendererProtocolHandler> {
	let installedHandler: DesktopRendererProtocolHandler | undefined;
	const handle = vi.fn<Protocol['handle']>((_scheme, handler) => {
		installedHandler = handler;
	});
	await installDesktopRendererScheme({ handle }, rendererRoot, fetchFile, fileProbe);
	if (installedHandler === undefined)
		throw new Error('The test protocol handler was not installed.');
	return installedHandler;
}

describe('Desktop renderer protocol', () => {
	it('registers one standard and secure scheme before Electron becomes ready', () => {
		const registerSchemesAsPrivileged = vi.fn<Protocol['registerSchemesAsPrivileged']>();

		registerDesktopRendererScheme({ registerSchemesAsPrivileged });

		expect(registerSchemesAsPrivileged).toHaveBeenCalledWith([
			{
				privileges: {
					codeCache: true,
					secure: true,
					standard: true,
					supportFetchAPI: true,
				},
				scheme: 'orchbee',
			},
		]);
	});

	it('serves exact build assets and falls back to index.html only for document routes', async () => {
		const root = path.resolve('/packaged/renderer');
		const files = new Set([path.join(root, 'index.html'), path.join(root, 'static/app.js')]);
		const fetchFile = vi.fn<(fileUrl: string) => Promise<Response>>(async (fileUrl) =>
			Promise.resolve(new Response(fileUrl)),
		);
		const handler = await installHandler(root, fetchFile, async (filePath) => files.has(filePath));

		const asset = await handler(new Request('orchbee://app/static/app.js'));
		expect(await asset.text()).toBe(pathToFileURL(path.join(root, 'static/app.js')).href);

		const route = await handler(
			new Request('orchbee://app/projects/project-1/workspaces/workspace-1', {
				headers: { accept: 'text/html,application/xhtml+xml' },
			}),
		);
		expect(await route.text()).toBe(pathToFileURL(path.join(root, 'index.html')).href);

		const missingAsset = await handler(
			new Request('orchbee://app/static/missing.js', {
				headers: { accept: '*/*' },
			}),
		);
		expect(missingAsset.status).toBe(404);
	});

	it('rejects foreign authorities, writes, and paths outside the renderer root', async () => {
		const fetchFile = vi.fn<(fileUrl: string) => Promise<Response>>();
		const root = path.resolve('/packaged/renderer');
		const handler = await installHandler(
			root,
			fetchFile,
			async (filePath) => filePath === path.join(root, 'index.html'),
		);

		expect(
			(await handler(new Request('orchbee://foreign/', { headers: { accept: 'text/html' } })))
				.status,
		).toBe(404);
		expect((await handler(new Request('orchbee://app/', { method: 'POST' }))).status).toBe(405);
		expect(
			(
				await handler(
					new Request('orchbee://app/%2E%2E%2Fsecrets', {
						headers: { accept: 'application/json' },
					}),
				)
			).status,
		).toBe(404);
		expect(fetchFile).not.toHaveBeenCalled();
	});

	it('fails startup when the packaged Web entry is missing', async () => {
		const handle = vi.fn<Protocol['handle']>();

		await expect(
			installDesktopRendererScheme(
				{ handle },
				'/packaged/renderer',
				async () => new Response(),
				async () => false,
			),
		).rejects.toThrow('packaged Web application entry');
		expect(handle).not.toHaveBeenCalled();
	});
});
