export const DESKTOP_HOST_PROTOCOL_VERSION = { major: 1, minor: 4 } as const;

export const DESKTOP_HOST_PROTOCOL_RANGE = {
	major: DESKTOP_HOST_PROTOCOL_VERSION.major,
	maximumMinor: DESKTOP_HOST_PROTOCOL_VERSION.minor,
	minimumMinor: DESKTOP_HOST_PROTOCOL_VERSION.minor,
} as const;
