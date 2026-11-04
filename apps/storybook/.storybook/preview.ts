import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
import { TooltipProvider } from '@orchbee/ui/components';
import { createElement } from 'react';
import type { Preview } from 'storybook-react-rsbuild';

const preview: Preview = {
	decorators: [(Story) => createElement(TooltipProvider, null, createElement(Story))],
	parameters: {
		a11y: {
			test: 'todo',
		},
		controls: {
			matchers: {
				color: /(background|color)$/iu,
				date: /Date$/u,
			},
		},
		layout: 'centered',
	},
	tags: ['autodocs'],
};

export default preview;
