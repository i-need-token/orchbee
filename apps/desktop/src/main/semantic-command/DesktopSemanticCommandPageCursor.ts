import { createHmac, timingSafeEqual } from 'node:crypto';

import {
	SemanticCommandCoordinatorGenerationSchema,
	SemanticCommandOperationIdSchema,
	type SemanticCommandCoordinatorGeneration,
	type SemanticCommandOperationId,
} from '@orchbee/semantic-command-protocol';
import { WorkbenchSharedDocumentRevisionSchema } from '@orchbee/workbench/document';
import { z } from 'zod';

const cursorPayloadSchema = z.strictObject({
	version: z.literal(1),
	coordinatorGeneration: SemanticCommandCoordinatorGenerationSchema,
	operationId: SemanticCommandOperationIdSchema,
	target: z.string().min(1).max(2048),
	revision: WorkbenchSharedDocumentRevisionSchema,
	offset: z.number().int().nonnegative(),
});

interface DesktopSemanticCommandPageCursorContext {
	readonly coordinatorGeneration: SemanticCommandCoordinatorGeneration;
	readonly operationId: SemanticCommandOperationId;
	readonly revision: z.infer<typeof WorkbenchSharedDocumentRevisionSchema>;
	readonly target: string;
}

export type DesktopSemanticCommandPageCursorResolution =
	| { readonly kind: 'resolved'; readonly offset: number }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'stale' };

export class DesktopSemanticCommandPageCursor {
	readonly #secret: Uint8Array;

	constructor(secret: Uint8Array) {
		if (secret.byteLength < 16) {
			throw new Error('Semantic command page cursor secret must contain at least 16 bytes.');
		}
		this.#secret = Uint8Array.from(secret);
	}

	create(context: DesktopSemanticCommandPageCursorContext, offset: number): string {
		const payload = cursorPayloadSchema.parse({ ...context, offset, version: 1 });
		const encodedPayload = Buffer.from(JSON.stringify(payload), 'utf8').toString('base64url');
		const signature = this.#sign(encodedPayload).toString('base64url');
		return `v1.${encodedPayload}.${signature}`;
	}

	resolve(
		cursor: string | undefined,
		context: DesktopSemanticCommandPageCursorContext,
	): DesktopSemanticCommandPageCursorResolution {
		if (cursor === undefined) return { kind: 'resolved', offset: 0 };
		const parts = cursor.split('.');
		const encodedPayload = parts[1];
		const encodedSignature = parts[2];
		if (
			parts.length !== 3 ||
			parts[0] !== 'v1' ||
			encodedPayload === undefined ||
			encodedSignature === undefined
		) {
			return { kind: 'invalid' };
		}
		let signature: Buffer;
		let payloadCandidate: unknown;
		try {
			signature = Buffer.from(encodedSignature, 'base64url');
			payloadCandidate = JSON.parse(Buffer.from(encodedPayload, 'base64url').toString('utf8'));
		} catch {
			return { kind: 'invalid' };
		}
		const expectedSignature = this.#sign(encodedPayload);
		if (
			signature.byteLength !== expectedSignature.byteLength ||
			!timingSafeEqual(signature, expectedSignature)
		) {
			return { kind: 'invalid' };
		}
		const parsed = cursorPayloadSchema.safeParse(payloadCandidate);
		if (!parsed.success) return { kind: 'invalid' };
		if (
			parsed.data.coordinatorGeneration !== context.coordinatorGeneration ||
			parsed.data.operationId !== context.operationId ||
			parsed.data.target !== context.target
		) {
			return { kind: 'invalid' };
		}
		if (parsed.data.revision !== context.revision) return { kind: 'stale' };
		return { kind: 'resolved', offset: parsed.data.offset };
	}

	#sign(encodedPayload: string): Buffer {
		return createHmac('sha256', this.#secret).update(encodedPayload, 'utf8').digest();
	}
}
