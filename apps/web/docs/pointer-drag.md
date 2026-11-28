# Pointer drag

`src/pointer-drag/` drags things inside the window with the pointer, such as the File Bar's rows. It
turns a press into a drag, shows the drag's label and the `grabbing` cursor, finds the drop target
under the pointer, and ends the drag with a drop or a cancel. It never decides what a drop does: a
target's `drop` invokes its owner's Action, as [Frontend Actions](../../../docs/frontend/actions.md)
asks of a gesture's committed result. It never sees files dragged in from the operating system,
which arrive as the browser's own drag events. The API and its contracts are the TSDoc of
`startPointerDrag`, `PointerDragKind`, `registerPointerDropTarget` and `pointer-drag-types.ts`.

## Dragging from a surface and dropping on one

- Call `startPointerDrag` from a source's `pointerdown` handler with the element that holds the
  press. `start` runs once the pointer has traveled the Tab Strip's 10 px, so make the cargo there,
  however large the selection is; return null to leave the press a click.
- Give each kind of payload one `PointerDragKind`, exported by the module that owns the payload. A
  source wraps its payload with `carry`, and a target reads it back with `read` and refuses a drag
  of a kind it does not take. Payloads never leave the window, so they stay live objects.
- Register a target's element with `registerPointerDropTarget` while it is mounted, and call the
  returned function when it unmounts. The innermost registered element under the pointer takes the
  drag. `over` runs on every frame while the drag is over the element, so cache its answer by what
  it depends on, as the virtual tree caches it by row and keys held.

## Decisions

- **Pointer events, not the browser's drag and drop**, as the File Bar proposal asks. A browser
  drag shows the system's cursor and a snapshot for its image and carries only strings; a pointer
  drag shows the `grabbing` cursor every Orchbee drag shows, draws its label with the theme, and
  carries live objects. Files from the system still need the browser's drag, which alone brings
  them in.
- **Every key belongs to the drag.** A key pressed during a drag changes only the modifiers its
  targets read, and Escape cancels; no binding beneath hears it, as none hears keys during a system
  drag.
- **A press that dragged never clicks.** Its release swallows the click it causes, after a drop
  and after a cancel by Escape or by the window losing focus alike, so the row beneath is neither
  selected nor opened. A pointer the browser took, or that pressed again before its release
  arrived, has no click left to swallow, so the next press clicks as usual.
- **The target under the pointer hears the drag on every frame**, not only on pointer moves, so a
  list that scrolls or a folder that expands beneath a still pointer updates its feedback, as the
  browser repeats `dragover`.
- **The label is a pill 10 px right of and below the pointer**, where VS Code places its drag
  image's label, above every layer, as the system draws a drag image.

## Browser behavior it relies on

- Chromium dispatches `click` in the same task as the `pointerup` that causes it, so a capture
  listener removed by a zero-delay timeout swallows exactly that click.
- While the source holds the pointer's capture, Chromium keeps `:hover` on the source, so nothing
  beneath shows hover during a drag and a target shows only its drop feedback.
