import type { FileContent, FileError, FileResult } from '../file-session';
import { imagePreviewMimeType } from './imagePreviewMimeType';

export type ImagePreviewDocumentState =
	| { readonly error: FileError; readonly kind: 'error' }
	| { readonly kind: 'image'; readonly size: number; readonly source: string }
	| { readonly kind: 'loading' };

interface ImagePreviewDocumentOptions {
	/** The file's path, whose extension names the image's media type. */
	readonly path: string;
	/** Reads the whole file as bytes: VS Code's image preview has no size limit. */
	readonly read: (signal: AbortSignal) => Promise<FileResult<FileContent>>;
}

const FAILED: ImagePreviewDocumentState = { error: { code: 'failed' }, kind: 'error' };

/** The bytes as a `BlobPart`, without a copy when they already sit in a plain `ArrayBuffer`. */
function blobPart(bytes: Uint8Array): Uint8Array<ArrayBuffer> {
	return bytes.buffer instanceof ArrayBuffer
		? new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength)
		: bytes.slice();
}

function dataUrl(bytes: Uint8Array, mimeType: string): Promise<string> {
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.addEventListener('load', () => {
			if (typeof reader.result === 'string') resolve(reader.result);
			else reject(new Error('The image did not read as a data URL.'));
		});
		reader.addEventListener('error', () => {
			reject(reader.error ?? new Error('The image could not be read.'));
		});
		reader.readAsDataURL(new Blob([blobPart(bytes)], { type: mimeType }));
	});
}

/**
 * One image file's content for its preview, as a `data:` URL: Orchbee's content security policy
 * admits `data:` images but not `blob:`. A reload keeps the shown image when its read fails, as
 * VS Code's does while a file is being written.
 */
export class ImagePreviewDocument {
	private controller: AbortController | null = null;
	private disposed = false;
	private readonly listeners = new Set<() => void>();
	private readonly mimeType: string;
	private readonly options: ImagePreviewDocumentOptions;
	private state: ImagePreviewDocumentState = { kind: 'loading' };

	public constructor(options: ImagePreviewDocumentOptions) {
		this.options = options;
		// Without a known type the content still reaches the image decoder, which then refuses it.
		this.mimeType = imagePreviewMimeType(options.path) ?? 'application/octet-stream';
		this.load();
	}

	public readonly getState = (): ImagePreviewDocumentState => this.state;

	public readonly subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	};

	public dispose(): void {
		if (this.disposed) return;
		this.disposed = true;
		this.controller?.abort();
		this.controller = null;
		this.listeners.clear();
	}

	/** Reads the file again after it changed on disk. */
	public reload(): void {
		if (!this.disposed) this.load();
	}

	/** Try Again after a failed read. */
	public retry(): void {
		if (this.state.kind === 'error') this.load();
	}

	private load(): void {
		this.controller?.abort();
		const controller = new AbortController();
		this.controller = controller;
		if (this.state.kind === 'error') this.update({ kind: 'loading' });
		void this.read(controller.signal).then((next) => {
			if (this.disposed || this.controller !== controller) return;
			this.controller = null;
			if (next.kind === 'error' && this.state.kind === 'image') return;
			this.update(next);
		});
	}

	private async read(signal: AbortSignal): Promise<ImagePreviewDocumentState> {
		try {
			const result = await this.options.read(signal);
			if (result.kind === 'error') return { error: result.error, kind: 'error' };
			if (result.value.kind !== 'bytes') return FAILED;
			const source = await dataUrl(result.value.bytes, this.mimeType);
			return { kind: 'image', size: result.value.bytes.byteLength, source };
		} catch {
			return FAILED;
		}
	}

	private update(next: ImagePreviewDocumentState): void {
		this.state = next;
		for (const listener of this.listeners) listener();
	}
}
