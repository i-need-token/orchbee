import { ProjectedTerminal, type ProjectedTerminalCommand } from '@orchbee/terminal';
import {
	AttachmentIdSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
} from '@orchbee/terminal-protocol';

const attachmentId = AttachmentIdSchema.parse('22'.repeat(16));
const creationId = CreationIdSchema.parse('33'.repeat(16));
const hostEpoch = HostEpochSchema.parse('11'.repeat(16));
const terminalId = TerminalIdSchema.parse('44'.repeat(16));

const root = document.createElement('main');
root.id = 'terminal-resize-ownership-witness';
root.dataset['witnessControl'] = 'chromium-resize-observer-with-explicit-lease-grant';
root.dataset['witnessLimitation'] = 'client-convergence-only-not-a-physical-phone';
root.dataset['witnessResizeCount'] = '0';
root.style.boxSizing = 'border-box';
root.style.height = '240px';
root.style.overflow = 'hidden';
root.style.position = 'relative';
root.style.width = '720px';

const grantButton = document.createElement('button');
grantButton.disabled = true;
grantButton.id = 'grant-resize-ownership';
grantButton.textContent = 'Grant resize ownership';

const disposeButton = document.createElement('button');
disposeButton.disabled = true;
disposeButton.id = 'dispose-resize-ownership-witness';
disposeButton.textContent = 'Dispose resize ownership witness';

document.body.replaceChildren(root, grantButton, disposeButton);
document.body.style.margin = '0';
Object.defineProperty(navigator, 'gpu', { configurable: true, value: undefined });

const commands: ProjectedTerminalCommand[] = [];
let lastResize: Extract<ProjectedTerminalCommand, { readonly kind: 'resize-terminal' }> | undefined;
const terminal = new ProjectedTerminal({
	attachmentId,
	creationId,
	hostEpoch,
	labels: { input: 'Resize ownership witness input', screen: 'Resize ownership witness screen' },
	port: {
		send: (command) => {
			commands.push(command);
			if (command.kind !== 'resize-terminal') return;
			lastResize = command;
			root.dataset['witnessResizeCount'] = String(resizeCommands().length);
			root.dataset['witnessCellPixelHeight'] = String(command.payload.cellPixelHeight);
			root.dataset['witnessCellPixelWidth'] = String(command.payload.cellPixelWidth);
			root.dataset['witnessColumns'] = String(command.payload.columns);
			root.dataset['witnessRows'] = String(command.payload.rows);
		},
	},
	terminalId,
});

grantButton.addEventListener('click', () => {
	grantButton.disabled = true;
	void grantOwnership().catch(reportFailure);
});

disposeButton.addEventListener('click', () => {
	disposeButton.disabled = true;
	try {
		terminal.dispose();
		root.dataset['witnessDisposeChildCount'] = String(root.childElementCount);
		root.dataset['witnessDisposed'] = 'true';
	} catch (error) {
		reportFailure(error);
	}
});

void startWitness();

async function startWitness(): Promise<void> {
	try {
		await terminal.open(root);
		await nextPaint();
		if (resizeCommands().length !== 0) {
			throw new Error('Observer emitted a resize before ownership grant.');
		}

		root.style.height = '480px';
		root.style.width = '320px';
		await nextPaint();
		await nextPaint();
		if (resizeCommands().length !== 0) {
			throw new Error('Viewport measurement implicitly claimed resize ownership.');
		}
		const bounds = root.getBoundingClientRect();
		root.dataset['witnessObserverHeight'] = String(bounds.height);
		root.dataset['witnessObserverWidth'] = String(bounds.width);
		root.dataset['witnessReady'] = 'true';
		grantButton.disabled = false;
	} catch (error) {
		reportFailure(error);
	}
}

async function grantOwnership(): Promise<void> {
	terminal.setResizeOwnership(true);
	await nextPaint();
	const resize = lastResize;
	if (resize === undefined || resizeCommands().length !== 1) {
		throw new Error('Lease grant did not publish exactly one immediate current measurement.');
	}
	const bounds = root.getBoundingClientRect();
	const terminalPixelWidth = resize.payload.columns * resize.payload.cellPixelWidth;
	const terminalPixelHeight = resize.payload.rows * resize.payload.cellPixelHeight;
	if (
		resize.payload.columns < 1 ||
		resize.payload.rows < 1 ||
		terminalPixelWidth > bounds.width * window.devicePixelRatio ||
		terminalPixelHeight > bounds.height * window.devicePixelRatio
	) {
		throw new Error('Lease grant published geometry outside the current observer surface.');
	}
	root.dataset['witnessTerminalPixelHeight'] = String(terminalPixelHeight);
	root.dataset['witnessTerminalPixelWidth'] = String(terminalPixelWidth);
	root.dataset['witnessComplete'] = 'true';
	disposeButton.disabled = false;
}

function resizeCommands(): Extract<
	ProjectedTerminalCommand,
	{ readonly kind: 'resize-terminal' }
>[] {
	return commands.filter(
		(command): command is Extract<ProjectedTerminalCommand, { readonly kind: 'resize-terminal' }> =>
			command.kind === 'resize-terminal',
	);
}

function nextPaint(): Promise<void> {
	return new Promise((resolve) => {
		requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
	});
}

function reportFailure(error: unknown): void {
	root.dataset['witnessFailure'] = error instanceof Error ? error.message : String(error);
}
