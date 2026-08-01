export const ProtocolErrorCode = {
	arithmeticOverflow: 'arithmetic-overflow',
	invalidField: 'invalid-field',
	invalidIdentifier: 'invalid-identifier',
	invalidJson: 'invalid-json',
	invalidMagic: 'invalid-magic',
	invalidUtf8: 'invalid-utf8',
	lengthMismatch: 'length-mismatch',
	limitExceeded: 'limit-exceeded',
	truncated: 'truncated',
	unknownRequiredFlag: 'unknown-required-flag',
	unknownRequiredMessage: 'unknown-required-message',
	unsupportedVersion: 'unsupported-version',
} as const;

export type ProtocolErrorCode = (typeof ProtocolErrorCode)[keyof typeof ProtocolErrorCode];

export interface ProtocolFailure {
	readonly code: ProtocolErrorCode;
	readonly detail: string;
	readonly ok: false;
}

export interface ProtocolSuccess<Value> {
	readonly ok: true;
	readonly value: Value;
}

export type ProtocolResult<Value> = ProtocolFailure | ProtocolSuccess<Value>;

export function protocolFailure(code: ProtocolErrorCode, detail: string): ProtocolFailure {
	return { code, detail, ok: false };
}

export function protocolSuccess<Value>(value: Value): ProtocolSuccess<Value> {
	return { ok: true, value };
}
