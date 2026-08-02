import { describe, expect, it } from 'vitest';

import { ProtocolErrorCode } from '../protocol-result';
import {
	CURRENT_PROTOCOL_RANGE,
	CURRENT_PROTOCOL_VERSION,
	negotiateProtocolVersion,
	ProtocolVersionRangeSchema,
} from '../version';

describe('protocol version negotiation', () => {
	it('publishes protocol 1.3 as the current compatible minor', () => {
		expect(CURRENT_PROTOCOL_VERSION).toEqual({ major: 1, minor: 3 });
		expect(CURRENT_PROTOCOL_RANGE).toEqual({ major: 1, maximumMinor: 3, minimumMinor: 0 });
	});

	it('chooses the highest overlapping minor on one major', () => {
		expect(
			negotiateProtocolVersion(
				{ major: 1, maximumMinor: 5, minimumMinor: 1 },
				{ major: 1, maximumMinor: 3, minimumMinor: 2 },
			),
		).toEqual({ ok: true, value: { major: 1, minor: 3 } });
	});

	it('rejects malformed and disjoint ranges', () => {
		expect(
			ProtocolVersionRangeSchema.safeParse({ major: 1, maximumMinor: 1, minimumMinor: 2 }).success,
		).toBe(false);
		expect(
			negotiateProtocolVersion(
				{ major: 1, maximumMinor: 1, minimumMinor: 0 },
				{ major: 1, maximumMinor: 3, minimumMinor: 2 },
			),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.unsupportedVersion, ok: false }));
	});
});
