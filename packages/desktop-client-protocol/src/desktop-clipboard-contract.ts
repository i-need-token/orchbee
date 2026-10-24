import { PROTOCOL_LIMITS } from '@orchbee/terminal-protocol';
import { z } from 'zod';

export const DesktopClipboardRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopClipboardRequestId'>();

export const DesktopClipboardTextSchema = z
	.string()
	.max(PROTOCOL_LIMITS.maxInputBytes)
	.superRefine((value, context) => {
		if (containsUnpairedSurrogate(value)) {
			context.addIssue({
				code: 'custom',
				message: 'Clipboard text contains an unpaired surrogate',
			});
			return;
		}
		if (new TextEncoder().encode(value).byteLength > PROTOCOL_LIMITS.maxInputBytes) {
			context.addIssue({
				code: 'custom',
				message: `Clipboard text exceeds ${PROTOCOL_LIMITS.maxInputBytes} UTF-8 bytes`,
			});
		}
	});

export const DesktopClipboardRendererCommandSchema = z.strictObject({
	requestId: DesktopClipboardRequestIdSchema,
	type: z.literal('desktop-clipboard-read-text-requested'),
});

const DesktopClipboardFailureCodeSchema = z.enum([
	'clipboard-read-failed',
	'clipboard-text-invalid',
	'desktop-quiescing',
	'desktop-clipboard-unavailable',
]);

const DesktopClipboardReadResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('completed'), text: DesktopClipboardTextSchema }),
	z.strictObject({ code: DesktopClipboardFailureCodeSchema, kind: z.literal('failed') }),
]);

export const DesktopClipboardMainEventSchema = z.strictObject({
	requestId: DesktopClipboardRequestIdSchema,
	result: DesktopClipboardReadResultSchema,
	type: z.literal('desktop-clipboard-read-text-responded'),
});

export type DesktopClipboardReadResult = z.infer<typeof DesktopClipboardReadResultSchema>;
export type DesktopClipboardRendererCommand = z.infer<typeof DesktopClipboardRendererCommandSchema>;
export type DesktopClipboardRequestId = z.infer<typeof DesktopClipboardRequestIdSchema>;

function containsUnpairedSurrogate(value: string): boolean {
	for (let index = 0; index < value.length; index += 1) {
		const code = value.charCodeAt(index);
		if (code >= 0xd800 && code <= 0xdbff) {
			const next = value.charCodeAt(index + 1);
			if (next >= 0xdc00 && next <= 0xdfff) {
				index += 1;
				continue;
			}
			return true;
		}
		if (code >= 0xdc00 && code <= 0xdfff) return true;
	}
	return false;
}
