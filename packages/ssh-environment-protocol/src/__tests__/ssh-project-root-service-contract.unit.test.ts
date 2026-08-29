import { describe, expect, it } from 'vitest';

import {
	SSH_PROJECT_ROOT_MAX_MESSAGE_BYTES,
	SSH_PROJECT_ROOT_MAX_PATH_BYTES,
	SshProjectRootInspectionRequestSchema,
	SshProjectRootInspectionResultSchema,
	decodeSshProjectRootInspectionRequest,
	decodeSshProjectRootInspectionResult,
	encodeSshProjectRootInspectionRequest,
	encodeSshProjectRootInspectionResult,
} from '../ssh-project-root-service-contract';

describe('SSH project-root service contract', () => {
	it('round-trips one strict request and each bounded result shape', () => {
		const request = SshProjectRootInspectionRequestSchema.parse({
			type: 'inspect-project-root',
			path: '/srv/orchbee/项目',
		});
		expect(
			decodeSshProjectRootInspectionRequest(encodeSshProjectRootInspectionRequest(request)),
		).toEqual({ ok: true, message: request });

		for (const result of [
			SshProjectRootInspectionResultSchema.parse({
				kind: 'resolved',
				canonicalPath: '/srv/orchbee/project',
			}),
			SshProjectRootInspectionResultSchema.parse({ kind: 'failed', reason: 'not-found' }),
		] as const) {
			expect(
				decodeSshProjectRootInspectionResult(encodeSshProjectRootInspectionResult(result)),
			).toEqual({ ok: true, message: result });
		}
	});

	it('rejects relative, controlled, malformed Unicode, oversized, and non-strict paths', () => {
		expect.hasAssertions();
		for (const path of [
			'relative/project',
			'/srv/project\nother',
			`/srv/${'a'.repeat(SSH_PROJECT_ROOT_MAX_PATH_BYTES)}`,
			'/srv/\ud800',
		]) {
			expect(
				SshProjectRootInspectionRequestSchema.safeParse({ type: 'inspect-project-root', path })
					.success,
			).toBe(false);
		}
		expect(
			SshProjectRootInspectionRequestSchema.safeParse({
				type: 'inspect-project-root',
				path: '/srv/project',
				unexpected: true,
			}).success,
		).toBe(false);
	});

	it('fails closed on empty, invalid UTF-8, invalid JSON, and oversized messages', () => {
		expect(decodeSshProjectRootInspectionRequest(new Uint8Array())).toEqual({
			ok: false,
			error: { code: 'message-empty' },
		});
		expect(decodeSshProjectRootInspectionRequest(Uint8Array.of(0xff))).toEqual({
			ok: false,
			error: { code: 'message-invalid' },
		});
		expect(decodeSshProjectRootInspectionRequest(new TextEncoder().encode('{}'))).toEqual({
			ok: false,
			error: { code: 'message-invalid' },
		});
		expect(
			decodeSshProjectRootInspectionRequest(new Uint8Array(SSH_PROJECT_ROOT_MAX_MESSAGE_BYTES + 1)),
		).toEqual({ ok: false, error: { code: 'message-too-large' } });
	});
});
