// "Make space" (DR-12), like Insert Space in OneNote: press where the space should start and drag; everything below
// that line moves along, in one undo step. A js-draw tool: it is enabled by a toolbar button and, being a primary
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
import { belowLine, clampShift } from './makeSpaceMath'

interface Drag {
  /** where the line is, in canvas units, and the pointer's start */
  line: number
  startY: number
  moved: AbstractComponent[]
  tops: number[]
  /** how far the lower part is shown moved right now */
  applied: number
}

export class MakeSpaceTool extends BaseTool {
  private drag: Drag | null = null

  /** `onLine` gets the height of the line on the page (client pixels), or null when no drag is going on. */
  constructor(
    private readonly editor: Editor,
    private readonly onLine: (clientY: number | null) => void,
  ) {
    super(editor.notifier, 'Make space')
    // js-draw tools start enabled, and a tool put into the primary group while enabled would switch the pen off
    this.setEnabled(false)
  }

  private clientY(screenY: number): number {
    const canvas = this.editor.getRootElement().querySelector('canvas')
    return (canvas?.getBoundingClientRect().top ?? 0) + screenY
  }

  private shift(dy: number, inHistory: boolean) {
    if (!this.drag || dy === 0) return
    const m = Mat33.translation(Vec2.of(0, dy))
    void this.editor.dispatch(
      uniteCommands(
        this.drag.moved.map((c) => c.transformBy(m)),
        100,
      ),
      inHistory,
    )
  }

  override onPointerDown(e: PointerDownEvt): boolean {
    if (!this.isEnabled() || e.allPointers.length > 1) return false
    const line = e.current.canvasPos.y
    const all = this.editor.image.getAllComponents()
    const moved = belowLine(all, (c) => c.getBBox().y, line)
    this.drag = { line, startY: line, moved, tops: moved.map((c) => c.getBBox().y), applied: 0 }
    this.onLine(this.clientY(e.current.screenPos.y))
    return true
  }

  override onPointerMove(e: PointerMoveEvt): void {
    const d = this.drag
    if (!d) return
    const dy = clampShift(e.current.canvasPos.y - d.startY, d.tops, d.line)
    this.shift(dy - d.applied, false)
    d.applied = dy
    // the line goes with the content: it stays at the top of the part that moves
    const screen = this.editor.viewport.canvasToScreen(Vec2.of(0, d.line + dy))
    this.onLine(this.clientY(screen.y))
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
