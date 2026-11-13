import { renderTuiLines } from '../tui-screen/renderTuiLines';
import { orchestrateFrame } from './orchestrateFrame';
import {
	FINAL_SHELL_STEP,
	PROMPT_STEPS,
	SHELL_LOOP_SECONDS,
	SHELL_MARKS,
	orchestrateViewOf,
} from './shell-script';

const TYPE_DELAY_SECONDS = 0.15;
const SECONDS_PER_CHARACTER = 0.034;
const TICK_MS = 50;

interface TypedCommand {
	readonly step: number;
	readonly target: HTMLElement;
	readonly caret: HTMLElement | null;
	readonly finished: readonly Node[];
	readonly text: string;
	typed: number;
}

// Loops the orchestrate shell: each command is typed, its output appears, and the terminal
// interface beside it follows. Under reduced motion the finished run stays as the page has it.
export function activateShell(document: Document, reducedMotion: boolean): void {
	const shell = document.querySelector<HTMLElement>('[data-shell]');
	const screen = document.querySelector('[data-tui="orchestrate"]');
	if (shell === null || screen === null || reducedMotion) return;
	const lines = [...shell.querySelectorAll<HTMLElement>('[data-from]')];
	const promptLine = shell.querySelector<HTMLElement>('[data-prompt-line]');
	const commands: TypedCommand[] = [...shell.querySelectorAll<HTMLElement>('[data-type]')].map(
		(target) => ({
			step: Number(target.dataset['type']),
			target,
			caret: target.parentElement?.querySelector<HTMLElement>('[data-typing-caret]') ?? null,
			finished: [...target.childNodes].map((node) => node.cloneNode(true)),
			text: target.textContent,
			typed: -1,
		}),
	);
	const start = Date.now();
	let shownStep = FINAL_SHELL_STEP;
	let shownView = orchestrateViewOf(FINAL_SHELL_STEP);

	const tick = (): void => {
		const seconds = ((Date.now() - start) / 1000) % SHELL_LOOP_SECONDS;
		const step = SHELL_MARKS.filter((mark) => seconds >= mark).length - 1;
		if (step !== shownStep) {
			shownStep = step;
			for (const line of lines) line.hidden = Number(line.dataset['from']) > step;
			if (promptLine !== null) promptLine.hidden = !PROMPT_STEPS.includes(step);
		}
		for (const command of commands) {
			const typing = command.step === step;
			const into = typing ? seconds - (SHELL_MARKS[step] ?? 0) - TYPE_DELAY_SECONDS : Infinity;
			const typed = Math.max(
				0,
				Math.min(command.text.length, Math.floor(into / SECONDS_PER_CHARACTER)),
			);
			if (command.caret !== null) command.caret.hidden = !typing;
			if (typed === command.typed) continue;
			command.typed = typed;
			if (typed === command.text.length)
				command.target.replaceChildren(...command.finished.map((node) => node.cloneNode(true)));
			else command.target.textContent = command.text.slice(0, typed);
		}
		const view = orchestrateViewOf(step);
		if (view !== shownView) {
			shownView = view;
			renderTuiLines(screen, orchestrateFrame(view));
		}
	};
	tick();
	window.setInterval(tick, TICK_MS);
}
