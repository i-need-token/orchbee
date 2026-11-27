import type { DesktopServerConnectionBootstrap } from '@orchbee/desktop-client-protocol';

/**
 * The File Bar is unfinished: development and test builds offer it, and a release offers none of
 * it until the owner releases it.
 */
export function offersFileBar(
	distribution: DesktopServerConnectionBootstrap['distribution'],
): boolean {
	return distribution !== 'release';
}
