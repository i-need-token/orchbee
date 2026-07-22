import type { ReactNode, SVGProps } from 'react';
export type TabStripIconName =
	| 'actor'
	| 'bluetooth'
	| 'camera'
	| 'chevron'
	| 'close'
	| 'collapse-sidebar'
	| 'default-favicon'
	| 'desktop-capture'
	| 'device'
	| 'expand-sidebar'
	| 'glic-actor'
	| 'glic-spark'
	| 'group-attention'
	| 'media-recording'
	| 'microphone'
	| 'more-vertical'
	| 'muted'
	| 'picture-in-picture'
	| 'plus'
	| 'projects'
	| 'sad-tab'
	| 'search'
	| 'shared-group'
	| 'spark'
	| 'spinner'
	| 'tab-capture'
	| 'unfocus-group'
	| 'usb'
	| 'volume'
	| 'vr';
export interface TabStripIconProps extends SVGProps<SVGSVGElement> {
	name: TabStripIconName;
}
export function TabStripIcon({ name, ...props }: TabStripIconProps): ReactNode {
	const shared: SVGProps<SVGSVGElement> = {
		viewBox: '0 0 24 24',
		width: 16,
		height: 16,
		fill: 'none',
		stroke: 'currentColor',
		strokeWidth: 2,
		strokeLinecap: 'round',
		strokeLinejoin: 'round',
		'aria-hidden': true,
		...props,
	};
	if (name === 'close')
		return (
			<svg {...shared}>
				<path d='M6 6l12 12M18 6 6 18' />
			</svg>
		);
	if (name === 'plus')
		return (
			<svg {...shared}>
				<path d='M12 5v14M5 12h14' />
			</svg>
		);
	if (name === 'volume')
		return (
			<svg {...shared}>
				<path d='M11 5 6 9H3v6h3l5 4V5Z' />
				<path d='M15 9a4 4 0 0 1 0 6M18 6a8 8 0 0 1 0 12' />
			</svg>
		);
	if (name === 'muted')
		return (
			<svg {...shared}>
				<path d='M11 5 6 9H3v6h3l5 4V5Z' />
				<path d='m16 10 5 5m0-5-5 5' />
			</svg>
		);
	if (name === 'microphone')
		return (
			<svg {...shared}>
				<rect x='9' y='3' width='6' height='11' rx='3' />
				<path d='M6 11a6 6 0 0 0 12 0M12 17v4m-4 0h8' />
			</svg>
		);
	if (name === 'camera')
		return (
			<svg {...shared}>
				<rect x='3' y='6' width='13' height='12' rx='2' />
				<path d='m16 10 5-3v10l-5-3' />
			</svg>
		);
	if (name === 'media-recording')
		return (
			<svg {...shared}>
				<circle cx='9' cy='12' r='5' fill='currentColor' stroke='none' />
				<path d='m15 10 5-3v10l-5-3' />
			</svg>
		);
	if (name === 'desktop-capture')
		return (
			<svg {...shared}>
				<rect x='3' y='4' width='18' height='13' rx='2' />
				<path d='M8 21h8m-4-4v4M8 10h8m-3-3 3 3-3 3' />
			</svg>
		);
	if (name === 'tab-capture')
		return (
			<svg {...shared}>
				<rect x='3' y='4' width='18' height='16' rx='2' />
				<path d='M3 8h18m-6 4 3 3-3 3m3-3h-7' />
			</svg>
		);
	if (name === 'picture-in-picture')
		return (
			<svg {...shared}>
				<rect x='3' y='5' width='18' height='14' rx='2' />
				<rect x='12' y='11' width='7' height='5' rx='1' fill='currentColor' stroke='none' />
			</svg>
		);
	if (name === 'actor')
		return (
			<svg {...shared}>
				<path d='m5 3 14 8-6 2-2 6Z' fill='currentColor' stroke='none' />
			</svg>
		);
	if (name === 'spark')
		return (
			<svg {...shared}>
				<path d='M12 2c.8 5.3 2.7 7.2 8 8-5.3.8-7.2 2.7-8 8-.8-5.3-2.7-7.2-8-8 5.3-.8 7.2-2.7 8-8Z' />

				<path d='M19 16c.3 2 1 2.7 3 3-2 .3-2.7 1-3 3-.3-2-1-2.7-3-3 2-.3 2.7-1 3-3Z' />
			</svg>
		);
	if (name === 'bluetooth')
		return (
			<svg {...shared}>
				<path d='m7 7 10 10-5 4V3l5 4L7 17' />
			</svg>
		);
	if (name === 'usb')
		return (
			<svg {...shared}>
				<path d='M12 3v15m0-15-2 2m2-2 2 2M12 12 7 9m0 0v-2m0 2H5m7 7 5-3m0 0v-2m0 2h2' />

				<circle cx='12' cy='20' r='1.5' fill='currentColor' stroke='none' />
			</svg>
		);
	if (name === 'device')
		return (
			<svg {...shared}>
				<rect x='5' y='4' width='14' height='16' rx='2' />
				<path d='M8 8h8m-8 4h8m-5 4h3' />
			</svg>
		);
	if (name === 'vr')
		return (
			<svg {...shared}>
				<path d='M3 8h18v7a3 3 0 0 1-3 3h-2l-2-4h-4l-2 4H6a3 3 0 0 1-3-3Z' />
				<circle cx='8' cy='12' r='1' fill='currentColor' stroke='none' />
				<circle cx='16' cy='12' r='1' fill='currentColor' stroke='none' />
			</svg>
		);
	if (name === 'chevron')
		return (
			<svg {...shared}>
				<path d='m8 10 4 4 4-4' />
			</svg>
		);
	if (name === 'more-vertical')
		return (
			<svg {...shared}>
				<circle cx='12' cy='5' r='1.5' fill='currentColor' stroke='none' />
				<circle cx='12' cy='12' r='1.5' fill='currentColor' stroke='none' />
				<circle cx='12' cy='19' r='1.5' fill='currentColor' stroke='none' />
			</svg>
		);
	if (name === 'collapse-sidebar')
		return (
			<svg {...shared}>
				<rect x='4' y='4' width='16' height='16' rx='2' />
				<path d='M9 4v16m7-11-3 3 3 3' />
			</svg>
		);
	if (name === 'expand-sidebar')
		return (
			<svg {...shared}>
				<rect x='4' y='4' width='16' height='16' rx='2' />
				<path d='M9 4v16m4-11 3 3-3 3' />
			</svg>
		);
	if (name === 'search')
		return (
			<svg {...shared}>
				<circle cx='11' cy='11' r='6' />
				<path d='m16 16 4 4' />
			</svg>
		);
	if (name === 'projects')
		return (
			<svg {...shared}>
				<rect x='4' y='6' width='12' height='12' rx='2' />
				<path d='M8 3h12v12m-8-5h5m-5 4h5' />
			</svg>
		);
	if (name === 'unfocus-group')
		return (
			<svg {...shared}>
				<path d='M9 4H4v5m11-5h5v5M9 20H4v-5m11 5h5v-5' />
				<path d='M8 12h8' />
			</svg>
		);
	if (name === 'shared-group')
		return (
			<svg {...shared}>
				<circle cx='9' cy='8' r='3' />
				<circle cx='17' cy='9' r='2.5' />
				<path d='M3.5 19c.5-4 2.3-6 5.5-6s5 2 5.5 6M14 14c3.5-.5 5.6 1.2 6.5 4' />
			</svg>
		);
	if (name === 'group-attention')
		return (
			<svg {...shared}>
				<circle cx='12' cy='12' r='7' fill='currentColor' stroke='none' />
			</svg>
		);
	if (name === 'default-favicon')
		return (
			<svg {...shared} viewBox='0 0 20 20'>
				<circle cx='10' cy='10' r='8' fill='#e8f0fe' stroke='#5f6368' strokeWidth='1.4' />

				<path
					d='M2.5 10h15M10 2c2.1 2.2 3.1 4.9 3.1 8S12.1 15.8 10 18M10 2C7.9 4.2 6.9 6.9 6.9 10s1 5.8 3.1 8'
					stroke='#5f6368'
					strokeWidth='1.1'
				/>
			</svg>
		);
	if (name === 'spinner')
		return (
			<svg {...shared} className={`ct-spinner ${props.className ?? ''}`}>
				<path d='M20 12a8 8 0 1 1-5.4-7.6' />
			</svg>
		);
	if (name === 'sad-tab')
		return (
			<svg {...shared} viewBox='0 0 20 20'>
				<path d='M4 2.5h9l3 3V17.5H4Z' fill='#e8eaed' stroke='#5f6368' strokeWidth='1.2' />

				<path
					d='M13 2.5v3h3M7 9h.01M13 9h.01M7.3 14c.8-1.2 1.7-1.8 2.7-1.8s1.9.6 2.7 1.8'
					stroke='#5f6368'
					strokeWidth='1.2'
				/>
			</svg>
		);
	if (name === 'glic-spark')
		return (
			<svg {...shared}>
				<path
					d='M12 1.8c.7 5.6 4.6 9.5 10.2 10.2-5.6.7-9.5 4.6-10.2 10.2C11.3 16.6 7.4 12.7 1.8 12 7.4 11.3 11.3 7.4 12 1.8Z'
					fill='currentColor'
				/>
			</svg>
		);
	return (
		<svg {...shared}>
			<path
				d='M4 5.5A2.5 2.5 0 0 1 6.5 3h11A2.5 2.5 0 0 1 20 5.5v8a2.5 2.5 0 0 1-2.5 2.5h-4l-4.4 4v-4H6.5A2.5 2.5 0 0 1 4 13.5v-8Zm4 3h8v-2H8v2Zm0 4h5v-2H8v2Z'
				fill='currentColor'
			/>
		</svg>
	);
}
