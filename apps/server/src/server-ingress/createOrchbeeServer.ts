import fastifyStatic from '@fastify/static';
import fastifyWebsocket from '@fastify/websocket';
import Fastify, { type FastifyInstance, type FastifyPluginAsync, type FastifyReply } from 'fastify';
import { lstatSync, readFileSync, realpathSync } from 'node:fs';
import { join } from 'node:path';

import { classifyServerRoute, type ServerFallbackRoute } from './classifyServerRoute';

export interface OrchbeeServerOptions {
	readonly applicationPlugin?: FastifyPluginAsync;
	readonly rendererRoot: string;
}

interface RendererDistribution {
	readonly indexHtml: Buffer;
	readonly staticRoot: string;
}

export function createOrchbeeServer(options: OrchbeeServerOptions): FastifyInstance {
	const renderer = loadRendererDistribution(options.rendererRoot);
	const server = Fastify({ logger: false });

	server.register(fastifyWebsocket);
	server.register(fastifyStatic, {
		cacheControl: true,
		immutable: true,
		index: false,
		maxAge: '1y',
		prefix: '/static/',
		redirect: false,
		root: renderer.staticRoot,
		serveDotFiles: false,
	});
	if (options.applicationPlugin !== undefined) server.register(options.applicationPlugin);

	server.addHook('onSend', (_request, reply, payload, done) => {
		reply.header('X-Content-Type-Options', 'nosniff');
		done(null, payload);
	});
	server.setNotFoundHandler((request, reply) => {
		let pathname: string;
		try {
			const queryIndex = request.url.indexOf('?');
			const encodedPathname = queryIndex === -1 ? request.url : request.url.slice(0, queryIndex);
			pathname = decodeURIComponent(encodedPathname);
		} catch {
			return reply
				.code(400)
				.header('Cache-Control', 'no-store')
				.send({ error: { code: 'invalid-request-path' } });
		}
		const route = classifyServerRoute({
			acceptsHtml: request.headers.accept?.toLowerCase().includes('text/html') ?? false,
			method: request.method,
			pathname,
		});
		return sendFallbackResponse(route, reply, renderer.indexHtml, request.method === 'HEAD');
	});

	return server;
}

function loadRendererDistribution(rendererRoot: string): RendererDistribution {
	const realRendererRoot = realpathSync(rendererRoot);
	const indexPath = join(realRendererRoot, 'index.html');
	const staticRoot = join(realRendererRoot, 'static');
	if (!lstatSync(indexPath).isFile()) {
		throw new Error('The Orchbee renderer entry must be a regular file.');
	}
	if (!lstatSync(staticRoot).isDirectory()) {
		throw new Error('The Orchbee renderer static root must be a directory.');
	}
	return {
		indexHtml: readFileSync(indexPath),
		staticRoot,
	};
}

function sendFallbackResponse(
	route: ServerFallbackRoute,
	reply: FastifyReply,
	indexHtml: Buffer,
	isHeadRequest: boolean,
): FastifyReply {
	if (route === 'document') {
		const documentReply = reply
			.header('Cache-Control', 'no-cache')
			.type('text/html; charset=utf-8');
		if (isHeadRequest) {
			return documentReply.header('Content-Length', indexHtml.byteLength).send();
		}
		return documentReply.send(indexHtml);
	}
	if (route === 'websocket-upgrade-required') {
		return reply
			.code(426)
			.header('Cache-Control', 'no-store')
			.header('Connection', 'Upgrade')
			.header('Upgrade', 'websocket')
			.send({ error: { code: route } });
	}
	return reply
		.code(404)
		.header('Cache-Control', 'no-store')
		.send({ error: { code: route } });
}
