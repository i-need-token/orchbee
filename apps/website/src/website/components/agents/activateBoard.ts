import { renderTuiLines } from '../tui-screen/renderTuiLines';
import { BOARD_AGENTS, boardFrame } from './board-frame';

const CHIME: readonly (readonly [frequency: number, offset: number])[] = [
	[880, 0],
	[1318.5, 0.12],
];

// The Agent board responds to clicks and the arrow keys, and the sound card plays a stand-in for
// the attention sound.
export function activateBoard(document: Document): void {
	const board = document.querySelector<HTMLElement>('[data-tui="board"]');
	if (board !== null) {
		const rows = [...board.querySelectorAll<HTMLButtonElement>('[data-board-row]')];
		let selected = 0;
		const select = (next: number, focus: boolean): void => {
			selected = next;
			renderTuiLines(board, boardFrame(next));
			rows.forEach((row, index) => row.setAttribute('aria-pressed', String(index === next)));
			if (focus) rows[next]?.focus();
		};
		rows.forEach((row, index) => row.addEventListener('click', () => select(index, false)));
		board.addEventListener('keydown', (event) => {
			if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
			event.preventDefault();
			const count = BOARD_AGENTS.length;
			select((selected + (event.key === 'ArrowDown' ? 1 : count - 1)) % count, true);
		});
	}

	let audio: AudioContext | undefined;
	for (const button of document.querySelectorAll('[data-action="chime"]')) {
		button.addEventListener('click', () => {
			audio ??= new AudioContext();
			if (audio.state === 'suspended') void audio.resume();
			const at = audio.currentTime + 0.02;
			for (const [frequency, offset] of CHIME) {
				const oscillator = audio.createOscillator();
				const gain = audio.createGain();
				oscillator.type = 'sine';
				oscillator.frequency.value = frequency;
				gain.gain.setValueAtTime(0.0001, at + offset);
				gain.gain.exponentialRampToValueAtTime(0.09, at + offset + 0.02);
				gain.gain.exponentialRampToValueAtTime(0.0001, at + offset + 0.9);
				oscillator.connect(gain);
				gain.connect(audio.destination);
				oscillator.start(at + offset);
				oscillator.stop(at + offset + 1);
			}
		});
	}
}
