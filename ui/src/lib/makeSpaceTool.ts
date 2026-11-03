// "Make space" (DR-12), like Insert Space in OneNote: press where the space should start and drag; everything below
// that line moves along (drag up or down), or everything to the right of it (drag sideways), in one undo step. The
// direction is taken from the first part of the drag. A js-draw tool: it is enabled by a toolbar button and, being a primary
// tool, disabled again when the person picks a pen. The moving is shown while dragging (not in the undo history) and
// is then made one real command, so a single undo takes it back.
import {
  BaseTool,
  Mat33,
  Vec2,
  uniteCommands,
  type AbstractComponent,
  type Editor,
  type PointerDownEvt,
  type PointerMoveEvt,
} from 'js-draw'
import { belowLine, clampShift, pickAxis, type Axis } from './makeSpaceMath'

/** Screen pixels the pointer must move before the direction is known (and the line is shown). */
const AXIS_THRESHOLD = 6

interface Drag {
  /** where the pointer went down: canvas units, and on the screen */
  start: Vec2
  startScreen: Vec2
  /** null until the drag is long enough to tell its direction; the rest is set with it */
  axis: Axis | null
  /** where the line is on its axis, in canvas units */
  line: number
  moved: AbstractComponent[]
  edges: number[]
  /** how far the far part is shown moved right now */
  applied: number
}

export class MakeSpaceTool extends BaseTool {
  private drag: Drag | null = null

  /** `onLine` gets the line on the page (its direction and its client pixel position), or null when no line is shown. */
  constructor(
    private readonly editor: Editor,
    private readonly onLine: (line: { axis: Axis; at: number } | null) => void,
  ) {
    super(editor.notifier, 'Make space')
    // js-draw tools start enabled, and a tool put into the primary group while enabled would switch the pen off
    this.setEnabled(false)
  }

  private clientAt(axis: Axis, screen: Vec2): number {
    const r = this.editor.getRootElement().querySelector('canvas')?.getBoundingClientRect()
    return axis === 'y' ? (r?.top ?? 0) + screen.y : (r?.left ?? 0) + screen.x
  }

  private shift(delta: number, inHistory: boolean) {
    const d = this.drag
    if (!d || !d.axis || delta === 0) return
    const m = Mat33.translation(d.axis === 'y' ? Vec2.of(0, delta) : Vec2.of(delta, 0))
    void this.editor.dispatch(
      uniteCommands(
        d.moved.map((c) => c.transformBy(m)),
        100,
      ),
      inHistory,
    )
  }

  override onPointerDown(e: PointerDownEvt): boolean {
    if (!this.isEnabled() || e.allPointers.length > 1) return false
    this.drag = {
      start: e.current.canvasPos,
      startScreen: e.current.screenPos,
      axis: null,
      line: 0,
      moved: [],
      edges: [],
      applied: 0,
    }
    return true
  }

  /** The direction is known: the line is where the pointer went down, and what is past it will move. */
  private begin(d: Drag, axis: Axis) {
    d.axis = axis
    d.line = axis === 'y' ? d.start.y : d.start.x
    const edge = (c: AbstractComponent) => (axis === 'y' ? c.getBBox().y : c.getBBox().x)
    d.moved = belowLine(this.editor.image.getAllComponents(), edge, d.line)
    d.edges = d.moved.map(edge)
  }

  override onPointerMove(e: PointerMoveEvt): void {
    const d = this.drag
    if (!d) return
    if (!d.axis) {
      const s = e.current.screenPos
      const axis = pickAxis(s.x - d.startScreen.x, s.y - d.startScreen.y, AXIS_THRESHOLD)
      if (!axis) return
      this.begin(d, axis)
    }
    const axis = d.axis!
    const pointer = axis === 'y' ? e.current.canvasPos.y - d.start.y : e.current.canvasPos.x - d.start.x
    const delta = clampShift(pointer, d.edges, d.line)
    this.shift(delta - d.applied, false)
    d.applied = delta
    // the line goes with the content: it stays at the edge of the part that moves
    const screen = this.editor.viewport.canvasToScreen(
      axis === 'y' ? Vec2.of(0, d.line + delta) : Vec2.of(d.line + delta, 0),
    )
    this.onLine({ axis, at: this.clientAt(axis, screen) })
  }

  /** Takes the shown move back and makes it one command that is in the history. */
  private finish(keep: boolean) {
    const d = this.drag
    if (!d) return
    const total = d.applied
    this.shift(-total, false)
    d.applied = 0
    if (keep) this.shift(total, true)
    this.drag = null
    this.onLine(null)
  }

  override onPointerUp(): void {
    this.finish(true)
  }

  override onGestureCancel(): void {
    this.finish(false)
  }
}
