export {};

declare module 'react' {
	interface CSSProperties {
		[propertyName: `--ob-comp-virtual-tree-${string}`]: string | number | undefined;
	}
}
