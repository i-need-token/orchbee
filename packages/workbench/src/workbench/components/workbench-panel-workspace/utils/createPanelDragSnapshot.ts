interface PanelDragSnapshot {
	readonly mount: (host: HTMLDivElement | null) => void;
}

function isHtmlElement(node: Node): node is HTMLElement {
	return node.nodeType === 1;
}

export function createPanelDragSnapshot(sourceElement: HTMLElement): PanelDragSnapshot | null {
	const snapshotNode = sourceElement.cloneNode(true);
	if (!isHtmlElement(snapshotNode)) {
		return null;
	}
	const snapshotElement = snapshotNode;
	snapshotElement.removeAttribute('data-focused');
	snapshotElement.removeAttribute('data-inactive');
	snapshotElement.removeAttribute('data-panel-id');
	snapshotElement.removeAttribute('data-slot');
	snapshotElement.setAttribute('aria-hidden', 'true');
	snapshotElement.setAttribute('inert', '');
	snapshotElement.setAttribute('data-panel-drag-snapshot-source', 'true');
	snapshotElement.querySelector('[data-slot="workbench-panel-focus-scrim"]')?.remove();
	for (const identifiedElement of snapshotElement.querySelectorAll('[id]')) {
		identifiedElement.removeAttribute('id');
	}
	const sourceCanvases = sourceElement.querySelectorAll('canvas');
	const snapshotCanvases = snapshotElement.querySelectorAll('canvas');
	for (const [index, sourceCanvas] of sourceCanvases.entries()) {
		const snapshotCanvas = snapshotCanvases[index];
		if (snapshotCanvas === undefined) {
			continue;
		}
		snapshotCanvas.width = sourceCanvas.width;
		snapshotCanvas.height = sourceCanvas.height;
		snapshotCanvas.getContext('2d')?.drawImage(sourceCanvas, 0, 0);
	}

	return {
		mount: (host) => {
			if (host !== null && host.firstElementChild !== snapshotElement) {
				host.replaceChildren(snapshotElement);
			}
		},
	};
}
