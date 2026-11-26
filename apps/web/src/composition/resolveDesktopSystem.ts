import type { DesktopSystem } from '../desktop-client';

/** The system a Desktop window runs on, from `navigator.platform`. */
export function resolveDesktopSystem(platform: string): DesktopSystem {
	if (/^(?:Mac|iPhone|iPad|iPod)/u.test(platform)) return 'macos';
	return platform.startsWith('Win') ? 'windows' : 'linux';
}
