export {};

declare module 'react' {
	interface CSSProperties {
		[propertyName: `--ob-comp-projected-terminal-${string}`]: string | number | undefined;
	}
}
