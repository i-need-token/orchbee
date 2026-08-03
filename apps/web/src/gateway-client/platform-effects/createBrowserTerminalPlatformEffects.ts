import type { ProjectedTerminalPlatformEffects } from '../../workbench-client';

export interface BrowserTerminalPlatformCapabilities {
	readonly openExternal: (target: string) => void;
	readonly writeClipboardText?: (text: string) => Promise<void>;
}

export function createBrowserTerminalPlatformEffects(
	capabilities: BrowserTerminalPlatformCapabilities,
): ProjectedTerminalPlatformEffects {
	return {
		openExternal: (target) => {
			if (!isPermittedTarget(target)) {
				return { code: 'link-policy-blocked', kind: 'failed', retryable: false };
			}
			try {
				capabilities.openExternal(new URL(target).href);
				return { kind: 'completed' };
			} catch {
				return { code: 'browser-link-open-failed', kind: 'failed', retryable: false };
			}
		},
		writeClipboardText: async (text) => {
			if (capabilities.writeClipboardText === undefined) {
				return { code: 'browser-clipboard-unavailable', kind: 'failed', retryable: false };
			}
			try {
				await capabilities.writeClipboardText(text);
				return { kind: 'completed' };
			} catch {
				return { code: 'browser-clipboard-write-failed', kind: 'failed', retryable: false };
			}
		},
	};
}

function isPermittedTarget(target: string): boolean {
	try {
		const protocol = new URL(target).protocol;
		return protocol === 'http:' || protocol === 'https:' || protocol === 'mailto:';
	} catch {
		return false;
	}
}
