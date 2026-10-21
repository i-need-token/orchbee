import { describe, expect, it } from 'vitest';

import { CURRENT_PROTOCOL_VERSION, ProtocolVersionSchema } from '../version';

describe('protocol version', () => {
	it('validates only the version shape and leaves exact matching to the handshake', () => {
		expect(ProtocolVersionSchema.safeParse(CURRENT_PROTOCOL_VERSION).success).toBe(true);
		expect(ProtocolVersionSchema.safeParse({ major: 1, minimumMinor: 0 }).success).toBe(false);
		expect(ProtocolVersionSchema.safeParse({ major: 0, minor: 1 }).success).toBe(false);
	});
});
