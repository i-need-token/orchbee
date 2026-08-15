import { randomBytes } from 'node:crypto';
import { chmod, mkdir, open, readFile, rename, stat, unlink } from 'node:fs/promises';
import path from 'node:path';

import { WorkbenchSharedDocumentRevisionSchema } from '@orchbee/workbench/document';
import { z } from 'zod';

const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

const DesktopDevelopmentRestartPlanSchema = z.discriminatedUnion('state', [
	z.strictObject({
		generation: z.number().int().positive(),
		state: z.literal('pending'),
		version: z.literal(1),
	}),
	z.strictObject({
		documentRevision: WorkbenchSharedDocumentRevisionSchema,
		generation: z.number().int().positive(),
		state: z.literal('ready'),
		version: z.literal(1),
	}),
]);

export type DesktopDevelopmentRestartPlan = z.infer<typeof DesktopDevelopmentRestartPlanSchema>;

export type DesktopDevelopmentRestartPlanLoadResult =
	| { readonly kind: 'available'; readonly plan: DesktopDevelopmentRestartPlan }
	| { readonly kind: 'invalid' | 'missing' | 'unavailable' };

export interface DesktopDevelopmentRestartPlanStoreOptions {
	readonly randomToken?: () => string;
}

export class DesktopDevelopmentRestartPlanStore {
	readonly #filePath: string;
	readonly #randomToken: () => string;
	#operationTail = Promise.resolve();

	constructor(filePath: string, options: DesktopDevelopmentRestartPlanStoreOptions = {}) {
		if (!path.isAbsolute(filePath)) {
			throw new Error('Desktop development restart plan path must be absolute.');
		}
		this.#filePath = filePath;
		this.#randomToken = options.randomToken ?? (() => randomBytes(16).toString('hex'));
	}

	load(): Promise<DesktopDevelopmentRestartPlanLoadResult> {
		return this.#enqueue(() => this.#loadCurrent());
	}

	writePending(generation: number): Promise<void> {
		const plan = DesktopDevelopmentRestartPlanSchema.parse({
			generation,
			state: 'pending',
			version: 1,
		});
		return this.#enqueue(() => this.#write(plan));
	}

	markReady(documentRevision: string): Promise<boolean> {
		const revision = WorkbenchSharedDocumentRevisionSchema.parse(documentRevision);
		return this.#enqueue(async () => {
			const loaded = await this.#loadCurrent();
			if (loaded.kind !== 'available') return false;
			if (loaded.plan.state === 'ready') {
				return loaded.plan.documentRevision === revision;
			}
			await this.#write({
				documentRevision: revision,
				generation: loaded.plan.generation,
				state: 'ready',
				version: 1,
			});
			return true;
		});
	}

	remove(): Promise<void> {
		return this.#enqueue(async () => {
			await unlink(this.#filePath).catch((error: unknown) => {
				if (!isNodeError(error) || error.code !== 'ENOENT') throw error;
			});
		});
	}

	#enqueue<Result>(operation: () => Promise<Result>): Promise<Result> {
		const result = this.#operationTail.then(operation);
		this.#operationTail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}

	async #loadCurrent(): Promise<DesktopDevelopmentRestartPlanLoadResult> {
		try {
			const metadata = await stat(this.#filePath);
			if (!metadata.isFile()) return { kind: 'invalid' };
			const encoded = await readFile(this.#filePath);
			let value: unknown;
			try {
				value = JSON.parse(utf8Decoder.decode(encoded));
			} catch {
				return { kind: 'invalid' };
			}
			const parsed = DesktopDevelopmentRestartPlanSchema.safeParse(value);
			return parsed.success ? { kind: 'available', plan: parsed.data } : { kind: 'invalid' };
		} catch (error: unknown) {
			if (isNodeError(error) && error.code === 'ENOENT') return { kind: 'missing' };
			return { kind: 'unavailable' };
		}
	}

	async #write(plan: DesktopDevelopmentRestartPlan): Promise<void> {
		const canonical = DesktopDevelopmentRestartPlanSchema.parse(plan);
		const encoded = Buffer.from(`${JSON.stringify(canonical)}\n`, 'utf8');
		const directory = path.dirname(this.#filePath);
		await mkdir(directory, { mode: 0o700, recursive: true });
		if (process.platform !== 'win32') await chmod(directory, 0o700);
		const temporaryPath = `${this.#filePath}.${this.#randomToken()}.tmp`;
		let temporaryCreated = false;
		try {
			const temporary = await open(temporaryPath, 'wx', 0o600);
			temporaryCreated = true;
			try {
				await temporary.writeFile(encoded);
				await temporary.sync();
			} finally {
				await temporary.close();
			}
			await rename(temporaryPath, this.#filePath);
		} catch (error: unknown) {
			if (temporaryCreated) await unlink(temporaryPath).catch(() => undefined);
			throw error;
		}
		if (process.platform !== 'win32') {
			await chmod(this.#filePath, 0o600);
			const directoryHandle = await open(directory, 'r');
			try {
				await directoryHandle.sync();
			} finally {
				await directoryHandle.close();
			}
		}
	}
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
