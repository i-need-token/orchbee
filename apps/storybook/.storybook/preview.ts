import '@orchbee/ui/styles.css';
import type { Preview } from 'storybook-react-rsbuild';

const preview: Preview = {
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
