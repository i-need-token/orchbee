import { lstat } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import type { Protocol } from 'electron';

const DESKTOP_RENDERER_SCHEME = 'orchbee';
export const DESKTOP_RENDERER_URL = `${DESKTOP_RENDERER_SCHEME}://app/`;

type DesktopRendererFileFetcher = (fileUrl: string) => Promise<Response>;
type DesktopRendererFileProbe = (filePath: string) => Promise<boolean>;

export function registerDesktopRendererScheme(
	protocol: Pick<Protocol, 'registerSchemesAsPrivileged'>,
): void {
	protocol.registerSchemesAsPrivileged([
		{
			privileges: {
				codeCache: true,
				secure: true,
				standard: true,
				supportFetchAPI: true,
			},
			scheme: DESKTOP_RENDERER_SCHEME,
		},
	]);
}

export async function installDesktopRendererScheme(
	protocol: Pick<Protocol, 'handle'>,
	rendererRoot: string,
	fetchFile: DesktopRendererFileFetcher,
	fileProbe: DesktopRendererFileProbe = isRegularFile,
): Promise<void> {
	const root = path.resolve(rendererRoot);
	const indexPath = path.join(root, 'index.html');
	if (!(await fileProbe(indexPath))) {
		throw new Error('The packaged Web application entry is unavailable.');
	}
	protocol.handle(
		DESKTOP_RENDERER_SCHEME,
		createDesktopRendererProtocolHandler(root, fetchFile, fileProbe),
	);
}

function createDesktopRendererProtocolHandler(
	rendererRoot: string,
	fetchFile: DesktopRendererFileFetcher,
	fileProbe: DesktopRendererFileProbe = isRegularFile,
): (request: Request) => Promise<Response> {
	const root = path.resolve(rendererRoot);
	const indexPath = path.join(root, 'index.html');
	return async (request) => {
		if (request.method !== 'GET') return new Response(null, { status: 405 });
		const requestUrl = parseDesktopRendererUrl(request.url);
		if (requestUrl === null) return new Response(null, { status: 404 });
		const candidate = resolveRendererFilePath(root, requestUrl.pathname);
		if (candidate !== null && (await fileProbe(candidate))) {
			return fetchFile(pathToFileURL(candidate).href);
		}
		if (!request.headers.get('accept')?.includes('text/html')) {
			return new Response(null, { status: 404 });
		}
		return fetchFile(pathToFileURL(indexPath).href);
	};
}

/** Only the packaged scheme and a loopback development server may hold the renderer document. */
export function isPermittedDesktopRendererUrl(url: URL): boolean {
	if (url.username !== '' || url.password !== '') return false;
	return (
		(url.protocol === 'http:' && url.hostname === '127.0.0.1') ||
		(url.protocol === `${DESKTOP_RENDERER_SCHEME}:` && url.hostname === 'app' && url.port === '')
	);
}

export function isSameDesktopRendererOrigin(value: string, rendererUrl: URL): boolean {
	let navigationUrl: URL;
	try {
		navigationUrl = new URL(value);
	} catch {
		return false;
	}
	return (
		isPermittedDesktopRendererUrl(navigationUrl) &&
		navigationUrl.protocol === rendererUrl.protocol &&
		navigationUrl.hostname === rendererUrl.hostname &&
		navigationUrl.port === rendererUrl.port
	);
}

function parseDesktopRendererUrl(value: string): URL | null {
	let url: URL;
	try {
		url = new URL(value);
	} catch {
		return null;
	}
	return url.protocol === `${DESKTOP_RENDERER_SCHEME}:` &&
		url.hostname === 'app' &&
		url.username === '' &&
		url.password === '' &&
		url.port === ''
		? url
		: null;
}

function resolveRendererFilePath(root: string, pathname: string): string | null {
	let decodedPath: string;
	try {
		decodedPath = decodeURIComponent(pathname);
	} catch {
		return null;
	}
	if (decodedPath.includes('\0')) return null;
	const relativePath = decodedPath.replace(/^[/\\]+/u, '');
	const candidate = path.resolve(root, relativePath);
	const relativeCandidate = path.relative(root, candidate);
	return relativeCandidate === '' ||
		relativeCandidate.startsWith(`..${path.sep}`) ||
		path.isAbsolute(relativeCandidate)
		? null
		: candidate;
}

async function isRegularFile(filePath: string): Promise<boolean> {
	try {
		return (await lstat(filePath)).isFile();
	} catch {
		return false;
	}
}
