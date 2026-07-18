import { designTokenNames, responsiveBreakpoints } from './design-token-names.generated';

export { designTokenNames, responsiveBreakpoints };

export type DesignTokenName = (typeof designTokenNames)[number];
export type DesignTokenReference = `var(${DesignTokenName})`;

export function designToken(name: DesignTokenName): DesignTokenReference {
	return `var(${name})`;
}
