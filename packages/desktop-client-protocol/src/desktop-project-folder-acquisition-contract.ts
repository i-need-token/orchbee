import { z } from 'zod';

export const DesktopProjectFolderAcquisitionRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopProjectFolderAcquisitionRequestId'>();

export const DesktopProjectFolderPathSchema = z
	.string()
	.min(1)
	.max(32_768)
	.refine((value) => !value.includes('\0'), 'Folder paths cannot contain NUL bytes.');

export const DesktopNewProjectFolderNameSchema = z
	.string()
	.min(1)
	.max(255)
	.refine((name) => {
		const stem = name.split('.')[0]?.trimEnd() ?? '';
		return (
			name.trim().length > 0 &&
			name.trimEnd() === name &&
			!/[\uD800-\uDFFF]/u.test(name) &&
			new TextEncoder().encode(name).length <= 255 &&
			!/[<>:"/\\|?*]/u.test(name) &&
			![...name].some(
				(character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127,
			) &&
			!/[. ]$/u.test(name) &&
			!/^(?:con|prn|aux|nul|conin\$|conout\$|com[1-9¹²³]|lpt[1-9¹²³])$/iu.test(stem)
		);
	}, 'A new Folder name must be one legal child component.');

export const DesktopProjectFolderAcquisitionRendererCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		requestId: DesktopProjectFolderAcquisitionRequestIdSchema,
		type: z.literal('desktop-project-folder-selection-requested'),
	}),
	z.strictObject({
		requestId: DesktopProjectFolderAcquisitionRequestIdSchema,
		type: z.literal('desktop-project-folder-parent-requested'),
	}),
	z.strictObject({
		attempt: z.number().int().positive().max(Number.MAX_SAFE_INTEGER),
		name: z.string().max(1024),
		parentRequestId: DesktopProjectFolderAcquisitionRequestIdSchema,
		requestId: DesktopProjectFolderAcquisitionRequestIdSchema,
		type: z.literal('desktop-project-folder-creation-requested'),
	}),
	z.strictObject({
		requestId: DesktopProjectFolderAcquisitionRequestIdSchema,
		type: z.literal('desktop-project-folder-creation-acknowledged'),
	}),
	z.strictObject({
		requestId: DesktopProjectFolderAcquisitionRequestIdSchema,
		type: z.literal('desktop-project-folder-parent-released'),
	}),
]);

const DesktopProjectFolderAcquisitionFailureCodeSchema = z.enum([
	'busy',
	'already-exists',
	'control-unavailable',
	'dialog-failed',
	'invalid-name',
	'not-found',
	'permission-denied',
	'stale-parent',
	'validation-failed',
]);

const DesktopProjectFolderAcquisitionResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('cancelled') }),
	z.strictObject({ kind: z.literal('selected'), path: DesktopProjectFolderPathSchema }),
	z.strictObject({ kind: z.literal('parent-selected'), path: DesktopProjectFolderPathSchema }),
	z.strictObject({ kind: z.literal('created'), path: DesktopProjectFolderPathSchema }),
	z.strictObject({ kind: z.literal('creation-unknown'), path: DesktopProjectFolderPathSchema }),
	z.strictObject({
		code: DesktopProjectFolderAcquisitionFailureCodeSchema,
		kind: z.literal('created-unavailable'),
		path: DesktopProjectFolderPathSchema,
	}),
]);

export const DesktopProjectFolderAcquisitionMainEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		requestId: DesktopProjectFolderAcquisitionRequestIdSchema,
		result: DesktopProjectFolderAcquisitionResultSchema,
		type: z.literal('desktop-project-folder-acquisition-completed'),
	}),
	z.strictObject({
		code: DesktopProjectFolderAcquisitionFailureCodeSchema,
		requestId: DesktopProjectFolderAcquisitionRequestIdSchema,
		type: z.literal('desktop-project-folder-acquisition-failed'),
	}),
]);

export type DesktopProjectFolderAcquisitionFailureCode = z.infer<
	typeof DesktopProjectFolderAcquisitionFailureCodeSchema
>;

export type DesktopProjectFolderAcquisitionRendererCommand = z.infer<
	typeof DesktopProjectFolderAcquisitionRendererCommandSchema
>;
export type DesktopProjectFolderAcquisitionRequestId = z.infer<
	typeof DesktopProjectFolderAcquisitionRequestIdSchema
>;
export type DesktopProjectFolderAcquisitionResult = z.infer<
	typeof DesktopProjectFolderAcquisitionResultSchema
>;
