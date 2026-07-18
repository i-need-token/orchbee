import { FitAddon, init, Terminal, type IDisposable } from '@orchbee/terminal';
import * as React from 'react';

import type { WorkbenchSession } from '../sessions';
import styles from './terminal-pane.module.css';

export interface TerminalPaneProps {
	readonly session: WorkbenchSession;
	readonly active: boolean;
	readonly onFocus: () => void;
	readonly showHeader?: boolean;
}

type TerminalPaneState = 'starting' | 'ready' | 'failed';

function resolveTerminalColor(color: string): string {
	const canvas = document.createElement('canvas');
	canvas.width = 1;
	canvas.height = 1;

	const context = canvas.getContext('2d');
	if (context === null) throw new Error('The terminal theme could not be resolved.');

	context.fillStyle = color;
	context.fillRect(0, 0, 1, 1);
	const colorData = context.getImageData(0, 0, 1, 1).data;
	const red = colorData[0] ?? 0;
	const green = colorData[1] ?? 0;
	const blue = colorData[2] ?? 0;
	return `rgb(${red}, ${green}, ${blue})`;
}

function readTerminalAppearance(parent: HTMLDivElement): {
	readonly fontFamily: string;
	readonly theme: {
		readonly background: string;
		readonly cursor: string;
		readonly cursorAccent: string;
		readonly foreground: string;
	};
} {
	const computedStyles = getComputedStyle(parent);
	const background = resolveTerminalColor(computedStyles.backgroundColor);
	const foreground = resolveTerminalColor(computedStyles.color);
	const fontFamily = computedStyles.getPropertyValue('--ob-ref-font-family-mono').trim();

	return {
		fontFamily: fontFamily === '' ? 'ui-monospace, monospace' : fontFamily,
		theme: {
			background,
			cursor: foreground,
			cursorAccent: background,
			foreground,
		},
	};
}

export function TerminalPane({
	session,
	active,
	onFocus,
	showHeader = true,
}: TerminalPaneProps): React.ReactNode {
	const surfaceReference = React.useRef<HTMLDivElement>(null);
	const [paneState, setPaneState] = React.useState<TerminalPaneState>('starting');
	const [errorMessage, setErrorMessage] = React.useState('');

	React.useEffect(() => {
		const parent = surfaceReference.current;
		if (parent === null) return undefined;

		let cancelled = false;
		let terminal: Terminal | undefined;
		let inputSubscription: IDisposable | undefined;
		let resizeSubscription: IDisposable | undefined;
		let unsubscribeOutput: (() => void) | undefined;

		async function openTerminal(terminalParent: HTMLDivElement): Promise<void> {
			await init();
			if (cancelled) return;
			const appearance = readTerminalAppearance(terminalParent);

			const nextTerminal = new Terminal({
				cols: 80,
				rows: 24,
				cursorBlink: true,
				fontFamily: appearance.fontFamily,
				fontSize: 14,
				fontWeight: 400,
				fontWeightBold: 600,
				paddingBalance: 'equal',
				theme: appearance.theme,
			});
			const fitAddon = new FitAddon();
			nextTerminal.loadAddon(fitAddon);
			terminal = nextTerminal;
			inputSubscription = nextTerminal.onData((data) => session.terminal.write(data));
			resizeSubscription = nextTerminal.onResize((size) => session.terminal.resize(size));
			unsubscribeOutput = session.terminal.subscribeOutput((data) => nextTerminal.write(data));

			await nextTerminal.open(terminalParent);
			if (cancelled) {
				nextTerminal.dispose();
				return;
			}
			fitAddon.fit();
			fitAddon.observeResize();
			setPaneState('ready');
		}

		void openTerminal(parent).catch((error: unknown) => {
			if (cancelled) return;
			setErrorMessage(error instanceof Error ? error.message : 'The terminal could not start.');
			setPaneState('failed');
		});

		return () => {
			cancelled = true;
			unsubscribeOutput?.();
			inputSubscription?.dispose();
			resizeSubscription?.dispose();
			terminal?.dispose();
		};
	}, [session]);

	const rootClassName = active ? styles['activeRoot'] : styles['root'];
	return (
		<section
			className={rootClassName}
			aria-label={`${session.title} terminal`}
			onFocusCapture={onFocus}
		>
			{showHeader && (
				<div className={styles['header']}>
					<span
						className={styles['statusIndicator']}
						data-status={session.status}
						aria-hidden='true'
					/>
					<span className={styles['title']}>{session.title}</span>
					<span className={styles['status']}>{session.status}</span>
				</div>
			)}
			<div ref={surfaceReference} className={styles['surface']} />
			{paneState !== 'ready' && (
				<div className={styles['overlay']} role={paneState === 'failed' ? 'alert' : 'status'}>
					{paneState === 'failed' ? errorMessage : 'Starting terminal…'}
				</div>
			)}
		</section>
	);
}
