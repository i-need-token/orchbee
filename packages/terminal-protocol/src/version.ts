import { z } from 'zod';

import { ProtocolErrorCode, protocolFailure, protocolSuccess } from './protocol-result';
import type { ProtocolResult } from './protocol-result';

const protocolComponentSchema = z.number().int().min(0).max(65_535);

export const ProtocolVersionSchema = z.strictObject({
	major: protocolComponentSchema,
	minor: protocolComponentSchema,
});

export const ProtocolVersionRangeSchema = z
	.strictObject({
		major: protocolComponentSchema,
		maximumMinor: protocolComponentSchema,
		minimumMinor: protocolComponentSchema,
	})
	.refine((value) => value.minimumMinor <= value.maximumMinor, {
		message: 'minimumMinor must not exceed maximumMinor',
	});

export type ProtocolVersion = z.infer<typeof ProtocolVersionSchema>;
export type ProtocolVersionRange = z.infer<typeof ProtocolVersionRangeSchema>;

export const CURRENT_PROTOCOL_VERSION = {
	major: 1,
	minor: 3,
} as const satisfies ProtocolVersion;

export const CURRENT_PROTOCOL_RANGE = {
	major: CURRENT_PROTOCOL_VERSION.major,
	maximumMinor: CURRENT_PROTOCOL_VERSION.minor,
	minimumMinor: 0,
} as const satisfies ProtocolVersionRange;

export function negotiateProtocolVersion(
	clientRange: unknown,
	hostRange: unknown,
): ProtocolResult<ProtocolVersion> {
	const client = ProtocolVersionRangeSchema.safeParse(clientRange);
	const host = ProtocolVersionRangeSchema.safeParse(hostRange);
	if (!client.success || !host.success) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Protocol negotiation requires valid client and Host ranges',
		);
	}
	if (client.data.major !== host.data.major) {
		return protocolFailure(
			ProtocolErrorCode.unsupportedVersion,
			'Client and Host protocol majors do not overlap',
		);
	}

	const minimumMinor = Math.max(client.data.minimumMinor, host.data.minimumMinor);
	const maximumMinor = Math.min(client.data.maximumMinor, host.data.maximumMinor);
	if (minimumMinor > maximumMinor) {
		return protocolFailure(
			ProtocolErrorCode.unsupportedVersion,
			'Client and Host protocol minor ranges do not overlap',
		);
	}
	return protocolSuccess({ major: client.data.major, minor: maximumMinor });
}
