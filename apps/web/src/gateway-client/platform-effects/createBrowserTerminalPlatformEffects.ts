import { normalizePermittedExternalTerminalUrl } from '@orchbee/terminal';

import type { ProjectedTerminalPlatformEffects } from '../../workbench-client';

export interface BrowserTerminalPlatformCapabilities {
	readonly getClipboardTextWriter: () => ((text: string) => Promise<void>) | undefined;
	readonly hasTransientUserActivation: () => boolean;
	readonly isSecureContext: () => boolean;
	readonly openExternal: (target: string) => void;
}

type BrowserClipboardWriteCapability =
	| {
			readonly kind: 'available';
			readonly writeClipboardText: (text: string) => Promise<void>;
	  }
	| {
			readonly code:
				| 'browser-clipboard-capability-check-failed'
				| 'browser-clipboard-insecure-context'
				| 'browser-clipboard-unavailable'
				| 'browser-clipboard-user-activation-required';
			readonly kind: 'unavailable';
	  };

export function createBrowserTerminalPlatformEffects(
	capabilities: BrowserTerminalPlatformCapabilities,
): ProjectedTerminalPlatformEffects {
	return {
		isClipboardWriteAvailable: () =>
			browserClipboardWriteCapability(capabilities).kind === 'available',
		openExternal: (target) => {
			const normalizedTarget = normalizePermittedExternalTerminalUrl(target);
			if (normalizedTarget === null) {
				return { code: 'link-policy-blocked', kind: 'failed', retryable: false };
			}
			try {
				capabilities.openExternal(normalizedTarget);
				return { kind: 'completed' };
			} catch {
				return { code: 'browser-link-open-failed', kind: 'failed', retryable: false };
			}
		},
		writeClipboardText: async (text) => {
			const capability = browserClipboardWriteCapability(capabilities);
			if (capability.kind === 'unavailable') {
				return { code: capability.code, kind: 'failed', retryable: false };
			}
			try {
				await capability.writeClipboardText(text);
				return { kind: 'completed' };
			} catch {
				return { code: 'browser-clipboard-write-failed', kind: 'failed', retryable: false };
			}
		},
	};
}

function browserClipboardWriteCapability(
	capabilities: BrowserTerminalPlatformCapabilities,
): BrowserClipboardWriteCapability {
	try {
		if (!capabilities.isSecureContext()) {
			return { code: 'browser-clipboard-insecure-context', kind: 'unavailable' };
		}
		const writeClipboardText = capabilities.getClipboardTextWriter();
		if (writeClipboardText === undefined) {
			return { code: 'browser-clipboard-unavailable', kind: 'unavailable' };
		}
		if (!capabilities.hasTransientUserActivation()) {
			return { code: 'browser-clipboard-user-activation-required', kind: 'unavailable' };
		}
		return { kind: 'available', writeClipboardText };
	} catch {
		return { code: 'browser-clipboard-capability-check-failed', kind: 'unavailable' };
	}
}
