import { describe, expect, it } from 'vitest';

import { TERMINAL_FRAME_FORMAT } from '../frame';
import { TERMINAL_PROJECTION_BUFFER_FORMAT } from '../projection-buffer';
import { TERMINAL_ROW_WINDOW_FORMAT } from '../row-window';
import { CURRENT_PROTOCOL_VERSION, ProtocolVersionSchema } from '../version';

describe('protocol version', () => {
	it('publishes one exact current pre-release contract', () => {
		expect(CURRENT_PROTOCOL_VERSION).toEqual({ major: 0, minor: 0, patch: 12 });
	});

	it('validates only the version shape and leaves exact matching to the handshake', () => {
		expect(ProtocolVersionSchema.safeParse(CURRENT_PROTOCOL_VERSION).success).toBe(true);
		expect(ProtocolVersionSchema.safeParse({ major: 1, minimumMinor: 0 }).success).toBe(false);
		expect(ProtocolVersionSchema.safeParse({ major: 0, minor: 1 }).success).toBe(false);
	});

	it('publishes one exact current pre-release version for every binary format', () => {
		expect(TERMINAL_FRAME_FORMAT).toEqual({ major: 0, minor: 0, patch: 2 });
		expect(TERMINAL_PROJECTION_BUFFER_FORMAT).toEqual({ major: 0, minor: 0, patch: 1 });
		expect(TERMINAL_ROW_WINDOW_FORMAT).toEqual({ major: 0, minor: 0, patch: 1 });
	});
});
