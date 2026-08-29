import type { TerminalImagePreviewMediaType } from '@orchbee/terminal';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { useState } from 'react';

import { useTerminalImagePreviewSource } from './useTerminalImagePreviewSource';

interface TerminalImagePreviewCardState {
	readonly failed: boolean;
	readonly reportFailure: () => void;
	readonly source: string;
}

export function useTerminalImagePreviewCard(
	content: ArrayBuffer,
	mediaType: TerminalImagePreviewMediaType,
): TerminalImagePreviewCardState {
	const source = useTerminalImagePreviewSource(content, mediaType);
	const [failedSource, setFailedSource] = useState<string | null>(null);
	const reportFailure = useMemoizedFn(() => setFailedSource(source));
	return { failed: failedSource === source, reportFailure, source };
}
