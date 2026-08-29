import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import { ProjectedTerminalImagePreviewCard } from '../src/workbench-client/projected-terminal-surface/components/projected-terminal-image-preview-card/ProjectedTerminalImagePreviewCard';

const imageContent = decodeBase64(
	'iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAIAAAAlC+aJAAAAR0lEQVR4nO3PQQ0AIBDAsAP/nuGNAvZoFSzZOjNnyNiBdwPeDXg34N2AdwPeDXg34N2AdwPeDXg34N2AdwPeDXg34N2AdwPeDXg3uB3wAUbMAf8ym4QAAAAASUVORK5CYII=',
);

const meta = {
	title: 'Projected Terminal/Image Preview Card',
	component: ProjectedTerminalImagePreviewCard,
	decorators: [
		(Story) => (
			<div data-story-terminal-image-preview='true'>
				<style>{`[data-story-terminal-image-preview='true'] { position: relative; inline-size: min(90vw, 48rem); block-size: min(80vh, 30rem); overflow: hidden; border: 1px solid var(--ob-sys-color-border-default); border-radius: var(--ob-ref-radius-ui-md); background: var(--ob-sys-color-surface-canvas); }`}</style>
				<Story />
			</div>
		),
	],
	parameters: { layout: 'centered' },
} satisfies Meta<typeof ProjectedTerminalImagePreviewCard>;

export default meta;

type Story = StoryObj<typeof meta>;

export const BelowPath: Story = {
	args: {
		preview: {
			anchor: {
				height: 20,
				left: 72,
				surfaceHeight: 480,
				surfaceWidth: 768,
				top: 54,
				width: 10,
			},
			content: imageContent,
			kind: 'visible',
			mediaType: 'image/png',
			path: '/workspace/artifacts/agent-result.png',
		},
	},
};

export const NearBottomEdge: Story = {
	args: {
		preview: {
			...BelowPath.args?.preview,
			anchor: {
				height: 20,
				left: 680,
				surfaceHeight: 480,
				surfaceWidth: 768,
				top: 420,
				width: 10,
			},
			content: imageContent,
			kind: 'visible',
			mediaType: 'image/png',
			path: '/workspace/artifacts/a-very-long-agent-generated-image-filename.png',
		},
	},
};

function decodeBase64(value: string): ArrayBuffer {
	const binary = globalThis.atob(value);
	const content = new ArrayBuffer(binary.length);
	const bytes = new Uint8Array(content);
	for (let index = 0; index < binary.length; index += 1) {
		bytes[index] = binary.charCodeAt(index);
	}
	return content;
}
