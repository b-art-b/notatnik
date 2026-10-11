// Which block properties are shown as a property card (PR-6): not hidden ones, and for a task not the ones that
// have their own chips and indicators.
import type { BlockView, PropView } from './api'

const CHIP_KEYS = ['scheduled', 'deadline', 'priority', 'repeat']

export function shownProps(block: BlockView): PropView[] {
  return block.props.filter((p) => !p.hidden && !(block.task && CHIP_KEYS.includes(p.key)))
}
