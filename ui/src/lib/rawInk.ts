// A pen stroke that keeps what the hand wrote (DR-3). js-draw's own freehand builders fit a smooth curve to the
// points the pen gives, which moves the line by up to 3 screen pixels and rounds off small details of a handwriting.
// This builder keeps every point: the line passes through the middle of each segment and bends towards the points
// in between (a quadratic curve per point, the usual way to draw a polyline without corners), so nothing is dropped,
// fitted or straightened. The width follows the pressure of a stylus; a mouse stroke is one path of one width.
import {
  Color4,
  PathCommandType,
  Rect2,
  StrokeComponent,
  Vec2,
  Viewport,
  makeFreehandLineBuilder,
  type AbstractComponent,
  type AbstractRenderer,
  type Point2,
  type RenderablePathSpec,
  type StrokeDataPoint,
} from 'js-draw'

/** A piece of the stroke is drawn as one path while its width stays within this fraction of the first one's. */
const WIDTH_STEP = 0.08

type Curve = { kind: PathCommandType.QuadraticBezierTo; controlPoint: Point2; endPoint: Point2 }

interface Group {
  start: Point2
  commands: Curve[]
  first: number
  sum: number
}

export class RawInkBuilder {
  private pts: Point2[] = []
  /** Every point as the pen gave it, for the shape fit (DR-18): a stroke is only straightened when asked. */
  private raw: StrokeDataPoint[] = []
  private widths: number[] = []
  /** Where the part drawn so far ends: the middle of the last finished segment. */
  private anchor: Point2
  private closed: RenderablePathSpec[] = []
  private group: Group | null = null
  private box: { x0: number; y0: number; x1: number; y1: number }
  private maxWidth: number
  private readonly tolerance: number

  constructor(
    private readonly first: StrokeDataPoint,
    private readonly viewport: Viewport,
  ) {
    // positions are kept to a tenth of a screen pixel: nothing visible is lost, the file stays small
    this.tolerance = viewport.getSizeOfPixelOnCanvas() / 10
    const p = Viewport.roundPoint(first.pos, this.tolerance)
    this.pts.push(p)
    this.raw.push(first)
    this.widths.push(first.width)
    this.anchor = p
    this.box = { x0: p.x, y0: p.y, x1: p.x, y1: p.y }
    this.maxWidth = first.width
  }

  getBBox(): Rect2 {
    const { x0, y0, x1, y1 } = this.box
    return new Rect2(x0, y0, x1 - x0, y1 - y0).grownBy(this.maxWidth / 2)
  }

  /** "Snap shapes" (DR-18, off unless the person turns it on): the pen was held still at the end of the stroke, so
   * what was drawn is fitted to a line or another shape by js-draw's own fitter, which is fed the same points. It
   * answers with a shape, or null when the stroke is none. */
  async autocorrectShape(): Promise<AbstractComponent | null> {
    const stock = makeFreehandLineBuilder(this.first, this.viewport)
    for (const p of this.raw.slice(1)) stock.addPoint(p)
    return (await stock.autocorrectShape?.()) ?? null
  }

  inkTrailStyle() {
    const sum = this.widths.reduce((a, b) => a + b, 0)
    return { color: this.first.color, width: sum / this.widths.length }
  }

  addPoint(point: StrokeDataPoint): void {
    const p = Viewport.roundPoint(point.pos, this.tolerance)
    const last = this.pts[this.pts.length - 1]
    if (p.x === last.x && p.y === last.y) return
    this.pts.push(p)
    this.raw.push(point)
    this.widths.push(point.width)
    const b = this.box
    b.x0 = Math.min(b.x0, p.x)
    b.y0 = Math.min(b.y0, p.y)
    b.x1 = Math.max(b.x1, p.x)
    b.y1 = Math.max(b.y1, p.y)
    this.maxWidth = Math.max(this.maxWidth, point.width)
    // the segment before the last point is finished: its curve ends in the middle of it
    const k = this.pts.length - 2
    if (k >= 1) {
      const end = Vec2.of((this.pts[k].x + p.x) / 2, (this.pts[k].y + p.y) / 2)
      this.push(this.pts[k], end, (this.widths[k - 1] + this.widths[k]) / 2)
    }
  }

  /** Adds the curve to the open group; a clearly different width starts a new path. */
  private push(controlPoint: Point2, endPoint: Point2, width: number): void {
    const g = this.group
    if (g && Math.abs(width - g.first) > WIDTH_STEP * g.first) {
      this.closed.push(this.spec(g))
      this.group = null
    }
    if (!this.group) this.group = { start: this.anchor, commands: [], first: width, sum: 0 }
    this.group.commands.push({ kind: PathCommandType.QuadraticBezierTo, controlPoint, endPoint })
    this.group.sum += width
    this.anchor = endPoint
  }

  private spec(g: Group): RenderablePathSpec {
    return {
      startPoint: g.start,
      commands: g.commands,
      style: {
        fill: Color4.transparent,
        stroke: { color: this.first.color, width: g.sum / g.commands.length },
      },
    }
  }

  /** The stroke so far: the finished paths, the open one, and a last piece that ends at the newest point. */
  private specs(): RenderablePathSpec[] {
    const out = [...this.closed]
    const n = this.pts.length - 1
    const tailWidth = n >= 1 ? (this.widths[n - 1] + this.widths[n]) / 2 : this.widths[0]
    const tail: Curve = {
      kind: PathCommandType.QuadraticBezierTo,
      controlPoint: this.pts[n],
      endPoint: this.pts[n],
    }
    const g = this.group
    if (g && Math.abs(tailWidth - g.first) <= WIDTH_STEP * g.first) {
      out.push(this.spec({ ...g, commands: [...g.commands, tail], sum: g.sum + tailWidth }))
    } else {
      if (g) out.push(this.spec(g))
      // a single point is a dot: a path of no length with round ends
      out.push(this.spec({ start: this.anchor, commands: [tail], first: tailWidth, sum: tailWidth }))
    }
    return out
  }

  preview(renderer: AbstractRenderer): void {
    renderer.startObject(this.viewport.visibleRect)
    for (const s of this.specs()) renderer.drawPath(s)
    renderer.endObject()
  }

  build(): StrokeComponent {
    return new StrokeComponent(this.specs())
  }
}

/** The stroke factory of the handwriting pen. */
export const makeRawInkBuilder = (start: StrokeDataPoint, viewport: Viewport) =>
  new RawInkBuilder(start, viewport)
