import * as React from 'react';

import {
	readWorkbenchLeftPanelWidth,
	writeWorkbenchLeftPanelWidth,
	type WorkbenchLayoutStorage,
} from '../../workbench-layout-storage';

interface ResizableLayoutChangedMeta {
	readonly isUserInteraction: boolean;
}

interface ResizablePanelSize {
	readonly inPixels: number;
}

interface WorkbenchShellLayout {
	readonly defaultLeftPanelSize: string;
	readonly onLayoutChanged: (
		layout: Readonly<Record<string, number>>,
		meta: ResizableLayoutChangedMeta,
	) => void;
	readonly onLeftPanelResize: (size: ResizablePanelSize) => void;
}

export function useWorkbenchShellLayout(storage?: WorkbenchLayoutStorage): WorkbenchShellLayout {
	const [defaultLeftPanelWidth] = React.useState(() => readWorkbenchLeftPanelWidth(storage));
	const leftPanelWidth = React.useRef(defaultLeftPanelWidth);

	return {
		defaultLeftPanelSize: `${defaultLeftPanelWidth}px`,
		onLayoutChanged: (_layout, meta) => {
			if (meta.isUserInteraction) {
				writeWorkbenchLeftPanelWidth(storage, leftPanelWidth.current);
			}
		},
		onLeftPanelResize: (size) => {
			leftPanelWidth.current = size.inPixels;
		},
	};
}
