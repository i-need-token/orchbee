import type { TerminalImagePreviewMediaType } from '@orchbee/terminal';
import { useMemo } from 'react';

import { encodeTerminalImagePreviewSource } from '../utils/encodeTerminalImagePreviewSource';

export function useTerminalImagePreviewSource(
	content: ArrayBuffer,
	mediaType: TerminalImagePreviewMediaType,
): string {
	return useMemo(() => encodeTerminalImagePreviewSource(content, mediaType), [content, mediaType]);
}
