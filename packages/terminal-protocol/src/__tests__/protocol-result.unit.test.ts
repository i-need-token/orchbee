import { describe, expect, it } from 'vitest';

import { ProtocolErrorCode, protocolFailure, protocolSuccess } from '../protocol-result';

describe('protocol result', () => {
	it('keeps expected failures distinct from successful values', () => {
		expect(protocolSuccess(42)).toEqual({ ok: true, value: 42 });
		expect(protocolFailure(ProtocolErrorCode.invalidField, 'bad field')).toEqual({
			code: ProtocolErrorCode.invalidField,
			detail: 'bad field',
			ok: false,
		});
	});
});
