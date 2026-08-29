import { projectedTerminalMessages } from '@orchbee/i18n';

export const projectedTerminalMobileInputKeys = [
	{ intent: 'Escape', label: 'Esc', message: projectedTerminalMessages.mobileEscape },
	{ intent: 'Tab', label: 'Tab', message: projectedTerminalMessages.mobileTab },
	{ intent: 'ArrowLeft', label: '←', message: projectedTerminalMessages.mobileArrowLeft },
	{ intent: 'ArrowUp', label: '↑', message: projectedTerminalMessages.mobileArrowUp },
	{ intent: 'ArrowDown', label: '↓', message: projectedTerminalMessages.mobileArrowDown },
	{ intent: 'ArrowRight', label: '→', message: projectedTerminalMessages.mobileArrowRight },
] as const;
