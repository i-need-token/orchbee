import type { PointerDrag } from './pointer-drag-types';

/** What a drag carries, shown beside the pointer as `label`; its kind reads the payload back. */
export interface PointerDragCargo {
	readonly label: string;
}

const attachments = new WeakMap<PointerDragCargo, (drag: PointerDrag) => void>();

/**
 * A kind of payload that drags carry, such as the File Bar's resources. A source makes a drag's
 * cargo with `carry`, and a target reads the payload back with `read`, typed, or learns that the
 * drag carries another kind.
 */
export class PointerDragKind<P> {
	private readonly payloads = new WeakMap<PointerDrag, P>();

	/** Cargo holding `payload`, labelled `label` beside the pointer. */
	carry(payload: P, label: string): PointerDragCargo {
		const cargo: PointerDragCargo = { label };
		attachments.set(cargo, (drag) => {
			this.payloads.set(drag, payload);
		});
		return cargo;
	}

	/** The payload `drag` carries, or undefined when it carries another kind. */
	read(drag: PointerDrag): P | undefined {
		return this.payloads.get(drag);
	}
}

/** Hands a drag the payload of the cargo it starts with. */
export function attachCargo(cargo: PointerDragCargo, drag: PointerDrag): void {
	attachments.get(cargo)?.(drag);
}
