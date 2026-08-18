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

export const DesktopProjectFolderAcquisitionRendererCommandSchema = z.strictObject({
	requestId: DesktopProjectFolderAcquisitionRequestIdSchema,
	type: z.literal('desktop-project-folder-selection-requested'),
});

export const DesktopProjectFolderAcquisitionFailureCodeSchema = z.enum([
	'busy',
	'control-unavailable',
	'dialog-failed',
	'not-directory',
	'not-found',
	'permission-denied',
	'validation-failed',
]);

export const DesktopProjectFolderAcquisitionResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('cancelled') }),
	z.strictObject({ kind: z.literal('selected'), path: DesktopProjectFolderPathSchema }),
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
export type DesktopProjectFolderAcquisitionMainEvent = z.infer<
	typeof DesktopProjectFolderAcquisitionMainEventSchema
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
