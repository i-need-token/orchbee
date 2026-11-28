import * as React from 'react';

/**
 * Whether the Explorer shows, as VS Code asks whether a view's body is visible: it has a box, which
 * the File Bar takes away from the view it hides. A resize observer reports the box after layout.
 */
export function useExplorerShown(rootReference: React.RefObject<HTMLElement | null>): boolean {
	const [shown, setShown] = React.useState(false);
	React.useEffect(() => {
		const root = rootReference.current;
		const ownerWindow = root?.ownerDocument.defaultView;
		if (root === null || !ownerWindow) return;
		const observer = new ownerWindow.ResizeObserver((entries) => {
			const size = entries.at(-1)?.contentRect;
			if (size !== undefined) setShown(size.width > 0 && size.height > 0);
		});
		observer.observe(root);
		return () => {
			observer.disconnect();
		};
	}, [rootReference]);
	return shown;
}
