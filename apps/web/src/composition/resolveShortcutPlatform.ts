import type { WorkbenchShortcutPlatform } from '@orchbee/workbench';

export function resolveShortcutPlatform(platform: string): WorkbenchShortcutPlatform {
	return /^(?:Mac|iPhone|iPad|iPod)/u.test(platform) ? 'apple' : 'standard';
}
