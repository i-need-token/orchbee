import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	ExitResultSchema,
	HostEpochSchema,
	TerminalIdentitySchema,
	TerminalLaunchV1_1Schema,
	type TerminalIdentity,
} from '@orchbee/terminal-protocol';
import { TerminalPresentationIdSchema } from '@orchbee/workbench/terminal-presentation';
import { z } from 'zod';

export const TerminalBindingIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9._-]+$/u)
	.brand<'TerminalBindingId'>();

const plannedStateSchema = z.strictObject({ kind: z.literal('planned') });
const createRequestedStateSchema = z.strictObject({
	hostEpoch: HostEpochSchema,
	kind: z.literal('create-requested'),
});
const retainedStateSchema = z.strictObject({
	identity: TerminalIdentitySchema,
	kind: z.literal('retained'),
});
const exitedStateSchema = z.strictObject({
	exit: ExitResultSchema,
	finalRevision: CanonicalUint64TextSchema,
	identity: TerminalIdentitySchema,
	kind: z.literal('exited'),
});
const closingStateSchema = z.strictObject({
	identity: TerminalIdentitySchema,
	kind: z.literal('closing'),
});
const lostStateSchema = z.strictObject({
	kind: z.literal('lost'),
	previousIdentity: TerminalIdentitySchema.nullable(),
	reason: z.enum(['host-epoch-changed', 'identity-conflict', 'identity-missing']),
});

export const TerminalBindingSchema = z
	.strictObject({
		bindingId: TerminalBindingIdSchema,
		creationId: CreationIdSchema,
		launch: TerminalLaunchV1_1Schema,
		presentationId: TerminalPresentationIdSchema,
		state: z.discriminatedUnion('kind', [
			plannedStateSchema,
			createRequestedStateSchema,
			retainedStateSchema,
			exitedStateSchema,
			closingStateSchema,
			lostStateSchema,
		]),
	})
	.superRefine((binding, context) => {
		const identity = bindingIdentity(binding);
		if (identity !== null && identity.creationId !== binding.creationId) {
			context.addIssue({
				code: 'custom',
				message: 'Terminal identity must repeat the binding CreationId',
				path: ['state'],
			});
		}
	});

const TerminalBindingStoreDocumentSchema = z
	.strictObject({
		bindings: z.array(TerminalBindingSchema),
		version: z.literal(3),
	})
	.superRefine((document, context) => {
		const bindingIds = new Set<string>();
		const creationIds = new Set<string>();
		const presentationIds = new Set<string>();
		const terminalIdentities = new Set<string>();
		for (const [index, binding] of document.bindings.entries()) {
			if (index > 0) {
				const previous = document.bindings[index - 1];
				if (previous !== undefined && previous.bindingId >= binding.bindingId) {
					context.addIssue({
						code: 'custom',
						message: 'Bindings must use unique canonical BindingId order',
						path: ['bindings', index, 'bindingId'],
					});
				}
			}
			if (bindingIds.has(binding.bindingId)) {
				context.addIssue({
					code: 'custom',
					message: 'BindingId must be unique',
					path: ['bindings', index, 'bindingId'],
				});
			}
			bindingIds.add(binding.bindingId);
			if (creationIds.has(binding.creationId)) {
				context.addIssue({
					code: 'custom',
					message: 'CreationId must be unique across bindings',
					path: ['bindings', index, 'creationId'],
				});
			}
			creationIds.add(binding.creationId);
			if (presentationIds.has(binding.presentationId)) {
				context.addIssue({
					code: 'custom',
					message: 'TerminalPresentationId must be unique across bindings',
					path: ['bindings', index, 'presentationId'],
				});
			}
			presentationIds.add(binding.presentationId);
			const identity = bindingIdentity(binding);
			if (identity === null) continue;
			const identityKey = `${identity.hostEpoch}:${identity.terminalId}`;
			if (terminalIdentities.has(identityKey)) {
				context.addIssue({
					code: 'custom',
					message: 'Host terminal identity must be unique across bindings',
					path: ['bindings', index, 'state'],
				});
			}
			terminalIdentities.add(identityKey);
		}
	});

export const TerminalBindingStoreFileSchema = TerminalBindingStoreDocumentSchema;

export type TerminalBindingId = z.infer<typeof TerminalBindingIdSchema>;
export type TerminalBinding = z.infer<typeof TerminalBindingSchema>;
export type TerminalBindingStoreDocument = z.infer<typeof TerminalBindingStoreDocumentSchema>;

export type TerminalBindingStoreLoadResult =
	| { readonly document: TerminalBindingStoreDocument; readonly kind: 'available' }
	| { readonly kind: 'missing' }
	| { readonly kind: 'unavailable'; readonly reason: 'invalid' | 'read-failed' };

export interface TerminalBindingStore {
	load(): Promise<TerminalBindingStoreLoadResult>;
	put(binding: TerminalBinding): Promise<void>;
	remove(bindingId: TerminalBindingId): Promise<void>;
}

export function createTerminalBindingStoreDocument(
	bindings: readonly TerminalBinding[] = [],
): TerminalBindingStoreDocument {
	return TerminalBindingStoreDocumentSchema.parse({
		bindings: [...bindings].sort((left, right) =>
			compareBindingIds(left.bindingId, right.bindingId),
		),
		version: 3,
	});
}

export function findTerminalBinding(
	document: TerminalBindingStoreDocument,
	bindingId: TerminalBindingId,
): TerminalBinding | undefined {
	return document.bindings.find((binding) => binding.bindingId === bindingId);
}

export function putTerminalBinding(
	document: TerminalBindingStoreDocument,
	binding: TerminalBinding,
): TerminalBindingStoreDocument {
	const canonical = TerminalBindingSchema.parse(binding);
	return createTerminalBindingStoreDocument([
		...document.bindings.filter((candidate) => candidate.bindingId !== canonical.bindingId),
		canonical,
	]);
}

export function removeTerminalBinding(
	document: TerminalBindingStoreDocument,
	bindingId: TerminalBindingId,
): TerminalBindingStoreDocument {
	return createTerminalBindingStoreDocument(
		document.bindings.filter((binding) => binding.bindingId !== bindingId),
	);
}

export function terminalBindingIdentity(binding: TerminalBinding): TerminalIdentity | null {
	return bindingIdentity(binding);
}

function bindingIdentity(binding: TerminalBinding): TerminalIdentity | null {
	if ('identity' in binding.state) return binding.state.identity;
	return binding.state.kind === 'lost' ? binding.state.previousIdentity : null;
}

function compareBindingIds(left: TerminalBindingId, right: TerminalBindingId): number {
	if (left < right) return -1;
	return left === right ? 0 : 1;
}
