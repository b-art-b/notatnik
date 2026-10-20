// The questions of a template (`{{ask:Label}}`): `/template` shows a dialog and waits for the answers.
import { app } from '../stores/app.svelte'

type Answers = { label: string; value: string }[]
let resolver: ((a: Answers | null) => void) | null = null

/** Opens the dialog for `asks` and resolves with the answers, or `null` when it was cancelled. */
export function askQuestions(
  name: string,
  asks: { label: string; default: string }[],
): Promise<Answers | null> {
  resolver?.(null) // a dialog that is still open is replaced
  app.templateAsks = { name, asks }
  return new Promise((resolve) => (resolver = resolve))
}

/** Called by the dialog: the answers, or `null` to cancel. */
export function answerQuestions(a: Answers | null): void {
  app.templateAsks = null
  const r = resolver
  resolver = null
  r?.(a)
}
