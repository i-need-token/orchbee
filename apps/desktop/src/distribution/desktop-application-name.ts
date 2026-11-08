import { z } from 'zod';

const DESKTOP_APPLICATION_NAME_MAX_LENGTH = 64;
const DESKTOP_APPLICATION_NAME_PATTERN = /^[^"*/:<>?\\|\p{Cc}]+$/u;
const WINDOWS_RESERVED_APPLICATION_NAME_PATTERN =
	/^(?:aux|con|nul|prn|com[1-9]|lpt[1-9])(?:\.|$)/iu;

// Default display names of the development and test distributions. The profile module owns each
// distribution's identity; this module owns the application-name rules and their defaults.
export const DEFAULT_DESKTOP_DEVELOPMENT_APPLICATION_NAME = 'Orchbee Dev';
export const DEFAULT_DESKTOP_TEST_APPLICATION_NAME = 'Orchbee Test';

export const desktopApplicationNameSchema = z
	.string()
	.min(1)
	.max(DESKTOP_APPLICATION_NAME_MAX_LENGTH)
	.regex(DESKTOP_APPLICATION_NAME_PATTERN)
	.refine((value) => value === value.trim())
	.refine((value) => !value.endsWith('.'))
	.refine((value) => !WINDOWS_RESERVED_APPLICATION_NAME_PATTERN.test(value));

export function resolveDesktopApplicationNameFromEnvironment(
	environment: Readonly<Record<string, string | undefined>>,
	environmentKey: string,
	defaultName: string,
): string {
	const configuredName = environment[environmentKey];
	if (configuredName === undefined) return defaultName;
	return parseDesktopApplicationName(configuredName, environmentKey);
}

export function parseDesktopApplicationName(value: string, source: string): string {
	const result = desktopApplicationNameSchema.safeParse(value.normalize('NFD'));
	if (result.success) return result.data;
	throw new Error(
		`${source} must be 1-${DESKTOP_APPLICATION_NAME_MAX_LENGTH} characters, have no leading or trailing whitespace, and contain no path separators, control characters, or platform-invalid filename characters.`,
	);
}
