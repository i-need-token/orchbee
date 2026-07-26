import type { ReactNode } from 'react';
import { TabStripIconSvg } from './components/tab-strip-icon-svg/TabStripIconSvg';
import type { TabStripIconVariantClass } from './types/tab-strip-icon-variant-class';
export type TabStripIconName =
	| 'bluetooth'
	| 'camera'
	| 'chevron'
	| 'close'
	| 'collapse-sidebar'
	| 'default-favicon'
	| 'desktop-capture'
	| 'device'
	| 'expand-sidebar'
	| 'group-attention'
	| 'media-recording'
	| 'microphone'
	| 'more-vertical'
	| 'muted'
	| 'picture-in-picture'
	| 'plus'
	| 'sad-tab'
	| 'search'
	| 'spinner'
	| 'tab-capture'
	| 'usb'
	| 'volume'
	| 'vr';
export interface TabStripIconProps {
	name: TabStripIconName;
	variantClass?: TabStripIconVariantClass | undefined;
}
export function TabStripIcon({ name, variantClass }: TabStripIconProps): ReactNode {
	if (name === 'close')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<path d='M6 6l12 12M18 6 6 18' />
			</TabStripIconSvg>
		);
	if (name === 'plus')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<path d='M12 5v14M5 12h14' />
			</TabStripIconSvg>
		);
	if (name === 'volume')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<path d='M11 5 6 9H3v6h3l5 4V5Z' />
				<path d='M15 9a4 4 0 0 1 0 6M18 6a8 8 0 0 1 0 12' />
			</TabStripIconSvg>
		);
	if (name === 'muted')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<path d='M11 5 6 9H3v6h3l5 4V5Z' />
				<path d='m16 10 5 5m0-5-5 5' />
			</TabStripIconSvg>
		);
	if (name === 'microphone')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<rect x='9' y='3' width='6' height='11' rx='3' />
				<path d='M6 11a6 6 0 0 0 12 0M12 17v4m-4 0h8' />
			</TabStripIconSvg>
		);
	if (name === 'camera')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<rect x='3' y='6' width='13' height='12' rx='2' />
				<path d='m16 10 5-3v10l-5-3' />
			</TabStripIconSvg>
		);
	if (name === 'media-recording')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<circle cx='9' cy='12' r='5' fill='currentColor' stroke='none' />
				<path d='m15 10 5-3v10l-5-3' />
			</TabStripIconSvg>
		);
	if (name === 'desktop-capture')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<rect x='3' y='4' width='18' height='13' rx='2' />
				<path d='M8 21h8m-4-4v4M8 10h8m-3-3 3 3-3 3' />
			</TabStripIconSvg>
		);
	if (name === 'tab-capture')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<rect x='3' y='4' width='18' height='16' rx='2' />
				<path d='M3 8h18m-6 4 3 3-3 3m3-3h-7' />
			</TabStripIconSvg>
		);
	if (name === 'picture-in-picture')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<rect x='3' y='5' width='18' height='14' rx='2' />
				<rect x='12' y='11' width='7' height='5' rx='1' fill='currentColor' stroke='none' />
			</TabStripIconSvg>
		);
	if (name === 'bluetooth')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<path d='m7 7 10 10-5 4V3l5 4L7 17' />
			</TabStripIconSvg>
		);
	if (name === 'usb')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<path d='M12 3v15m0-15-2 2m2-2 2 2M12 12 7 9m0 0v-2m0 2H5m7 7 5-3m0 0v-2m0 2h2' />

				<circle cx='12' cy='20' r='1.5' fill='currentColor' stroke='none' />
			</TabStripIconSvg>
		);
	if (name === 'device')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<rect x='5' y='4' width='14' height='16' rx='2' />
				<path d='M8 8h8m-8 4h8m-5 4h3' />
			</TabStripIconSvg>
		);
	if (name === 'vr')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<path d='M3 8h18v7a3 3 0 0 1-3 3h-2l-2-4h-4l-2 4H6a3 3 0 0 1-3-3Z' />
				<circle cx='8' cy='12' r='1' fill='currentColor' stroke='none' />
				<circle cx='16' cy='12' r='1' fill='currentColor' stroke='none' />
			</TabStripIconSvg>
		);
	if (name === 'chevron')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<path d='m8 10 4 4 4-4' />
			</TabStripIconSvg>
		);
	if (name === 'more-vertical')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<circle cx='12' cy='5' r='1.5' fill='currentColor' stroke='none' />
				<circle cx='12' cy='12' r='1.5' fill='currentColor' stroke='none' />
				<circle cx='12' cy='19' r='1.5' fill='currentColor' stroke='none' />
			</TabStripIconSvg>
		);
	if (name === 'collapse-sidebar')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<rect x='4' y='4' width='16' height='16' rx='2' />
				<path d='M9 4v16m7-11-3 3 3 3' />
			</TabStripIconSvg>
		);
	if (name === 'expand-sidebar')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<rect x='4' y='4' width='16' height='16' rx='2' />
				<path d='M9 4v16m4-11 3 3-3 3' />
			</TabStripIconSvg>
		);
	if (name === 'search')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<circle cx='11' cy='11' r='6' />
				<path d='m16 16 4 4' />
			</TabStripIconSvg>
		);
	if (name === 'group-attention')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass}>
				<circle cx='12' cy='12' r='7' fill='currentColor' stroke='none' />
			</TabStripIconSvg>
		);
	if (name === 'default-favicon')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass} viewBox='0 0 20 20'>
				<circle cx='10' cy='10' r='8' fill='#e8f0fe' stroke='#5f6368' strokeWidth='1.4' />

				<path
					d='M2.5 10h15M10 2c2.1 2.2 3.1 4.9 3.1 8S12.1 15.8 10 18M10 2C7.9 4.2 6.9 6.9 6.9 10s1 5.8 3.1 8'
					stroke='#5f6368'
					strokeWidth='1.1'
				/>
			</TabStripIconSvg>
		);
	if (name === 'spinner')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass} spinner>
				<path d='M20 12a8 8 0 1 1-5.4-7.6' />
			</TabStripIconSvg>
		);
	if (name === 'sad-tab')
		return (
			<TabStripIconSvg iconName={name} variantClass={variantClass} viewBox='0 0 20 20'>
				<path d='M4 2.5h9l3 3V17.5H4Z' fill='#e8eaed' stroke='#5f6368' strokeWidth='1.2' />

				<path
					d='M13 2.5v3h3M7 9h.01M13 9h.01M7.3 14c.8-1.2 1.7-1.8 2.7-1.8s1.9.6 2.7 1.8'
					stroke='#5f6368'
					strokeWidth='1.2'
				/>
			</TabStripIconSvg>
		);
	return null;
}
