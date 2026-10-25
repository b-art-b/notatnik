<script lang="ts">
  // "Copy or move to another graph" (MG-2 to MG-4): pages, or blocks, from the graph of this pane to another graph. The
  // dialog asks the backend what would happen (`transfer_plan`, which writes nothing) and shows it before the person
  // chooses; the toast afterwards has an Undo.
  import { backend, backendFor, type TransferArgs, type TransferPlan } from '../api'
  import { app, errorMessage, navigate, toast } from '../stores/app.svelte'
  import { openDocs } from '../stores/pages.svelte'
  import { removeRecent } from '../stores/recent.svelte'
  import { known, refreshWorkspace } from '../stores/workspace.svelte'
  import { graphColorVar } from '../graphColor'

  // `app.transfer` is set when the dialog opens and never null while it is shown: `req` is a snapshot of it
  const req = app.transfer!
  // The notes are in this pane's graph, unless they were dropped here from the other pane (`req.from`).
  const from = req.from ?? backend.slot
  const here = from === backend.slot
  // The target is fixed when the notes were dropped on a pane (`req.to`).
  const fixedTo = req.to

  interface Target {
    key: string
    label: string
    /** Where it is: "open beside" or its folder. */
    where: string
    slot?: string
    path?: string
    color: number
  }

  void refreshWorkspace()
  // The other graphs: the ones open in a pane first, then the rest of this device's graphs (their folders are opened
  // for the transfer only). The graph of this pane is in `known.slots` too, and is left out.
  const targets = $derived.by(() => {
    const out: Target[] = []
    for (const s of known.slots)
      if (fixedTo !== undefined ? s.slot === fixedTo : s.slot !== from)
        out.push({ key: `slot:${s.slot}`, label: s.name, where: 'open beside', slot: s.slot, color: s.color })
    const open = new Set(known.slots.map((s) => s.graph_id))
    for (const g of known.graphs)
      if (fixedTo === undefined && g.exists && !open.has(g.id))
        out.push({ key: `path:${g.path}`, label: g.name, where: g.path, path: g.path, color: g.color })
    return out
  })

  let chosen = $state('')
  // the first target is chosen once the list is there; a later change of the list keeps the choice
  $effect(() => {
    if (!targets.some((t) => t.key === chosen)) chosen = targets[0]?.key ?? ''
  })
  const target = $derived(targets.find((t) => t.key === chosen))

  let moving = $state(false)
  let withSub = $state(false)
  let clash = $state<'keep_both' | 'append' | 'skip'>('keep_both')
  let targetPage = $state(req.targetPage ?? req.page)
  // the pages below the page: known to the pane the notes are in
  let hasSubpages = $state(req.hasSubpages ?? false)
  $effect(() => {
    if (req.kind !== 'pages' || req.hasSubpages !== undefined) return
    void backendFor(from)
      .call('subpages', { page: req.page })
      .then((r) => (hasSubpages = r.some((p) => !p.is_virtual)))
  })
  const sourceName = $derived(known.slots.find((s) => s.slot === from)?.name ?? app.graph?.name ?? '')
  const sourceColor = $derived(known.slots.find((s) => s.slot === from)?.color)
  let plan = $state<TransferPlan | null>(null)
  let error = $state('')
  let busy = $state(false)

  function args(): TransferArgs | null {
    if (!target) return null
    return {
      from,
      to_slot: target.slot,
      to_path: target.path,
      kind: req.kind,
      page: req.page,
      include_subpages: req.kind === 'pages' && withSub,
      blocks: req.blocks ?? [],
      target_page: req.kind === 'blocks' ? targetPage.trim() || req.page : undefined,
      remove: moving,
      clash,
    }
  }

  // The plan is asked again whenever a choice changes. `cancelled` drops an answer that arrives after a newer question.
  $effect(() => {
    const a = args()
    if (!a) return
    let cancelled = false
    const t = setTimeout(() => {
      backend
        .call('transfer_plan', a)
        .then((p) => {
          if (cancelled) return
          plan = p
          error = ''
        })
        .catch((e) => {
          if (cancelled) return
          plan = null
          error = errorMessage(e)
        })
    }, 150)
    return () => {
      cancelled = true
      clearTimeout(t)
    }
  })

  const count = (action: string) => plan?.assets.filter((a) => a.action === action).length ?? 0
  const showClash = $derived(
    req.kind === 'pages' && !!plan && plan.items.some((i) => i.renamed || i.action !== 'create'),
  )
  const doing = $derived(plan?.items.filter((i) => i.action !== 'skip') ?? [])

  function close() {
    app.transfer = null
  }

  /** What the sentence in the toast calls the notes. */
  function what(p: TransferPlan): string {
    const n = p.items.filter((i) => i.action !== 'skip')
    if (req.kind === 'blocks') return `${req.blocks?.length ?? 0} block${req.blocks?.length === 1 ? '' : 's'}`
    return n.length === 1 ? `“${n[0].source}”` : `${n.length} pages`
  }

  /** After notes left this graph: documents of the pages that are gone or changed are stale, the page on screen may be gone. */
  function leftThisGraph(names: string[]) {
    // the pane the notes were in hears about it by the events; this one only knows its own graph
    if (!here) return
    for (const n of names) removeRecent(n)
    for (const d of openDocs()) if (names.includes(d.name)) void d.load()
  }

  async function go() {
    const a = args()
    if (!a || busy) return
    busy = true
    error = ''
    try {
      const r = await backend.call('transfer_apply', a)
      const sources = r.plan.items.filter((i) => i.action !== 'skip').map((i) => i.source)
      close()
      if (moving) {
        leftThisGraph(req.kind === 'blocks' ? [req.page] : sources)
        if (here && req.kind === 'pages' && app.route.kind === 'page' && sources.includes(app.route.name))
          navigate({ kind: 'journals' })
      }
      const verb = moving ? 'Moved' : 'Copied'
      const text = `${verb} ${what(r.plan)} to ${r.plan.to_name}`
      toast(
        r.written.length ? text : 'Nothing was copied (the pages exist already)',
        r.written.length ? 'info' : 'error',
        r.undoable
          ? {
              label: 'Undo',
              run: async () => {
                try {
                  const left = await backend.call('transfer_undo', {})
                  leftThisGraph(sources)
                  if (left.items.length) toast(`Not all was taken back: ${left.items.join('; ')}`, 'error')
                  else toast('Taken back')
                } catch (e) {
                  toast(errorMessage(e), 'error')
                }
              },
            }
          : undefined,
      )
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation()
      close()
    }
  }
</script>

<div class="overlay" role="presentation" onclick={close}>
  <div
    class="dialog"
    role="dialog"
    aria-label="Copy or move to another graph"
    aria-modal="true"
    tabindex="-1"
    data-testid="transfer-dialog"
    onclick={(e) => e.stopPropagation()}
    onkeydown={key}
  >
    <h2>To another graph</h2>
    <p class="muted">
      {#if req.kind === 'blocks'}
        {req.blocks?.length} block{req.blocks?.length === 1 ? '' : 's'} of <strong>{req.page}</strong>, with
        what is below {req.blocks?.length === 1 ? 'it' : 'them'}, from
      {:else}
        <strong>{req.page}</strong>, from
      {/if}
      <span class="graph"
        ><span
          class="gdot"
          style:background={sourceColor === undefined ? undefined : graphColorVar(sourceColor)}
          aria-hidden="true"
        ></span><strong data-testid="transfer-source">{sourceName}</strong></span
      >.
    </p>

    {#if targets.length === 0}
      <p class="warn" data-testid="transfer-none">
        There is no other graph on this device yet. Add one from the graph menu.
      </p>
    {:else}
      <fieldset>
        <legend>To</legend>
        {#each targets as t (t.key)}
          <label class="choice">
            <input
              type="radio"
              name="target"
              value={t.key}
              bind:group={chosen}
              data-testid="transfer-target"
            />
            <span class="graph"
              ><span class="gdot" style:background={graphColorVar(t.color)} aria-hidden="true"
              ></span>{t.label}</span
            >
            <span class="where">{t.where}</span>
          </label>
        {/each}
      </fieldset>

      <fieldset>
        <legend>What happens</legend>
        <label class="choice"
          ><input type="radio" bind:group={moving} value={false} data-testid="transfer-copy" /> Copy: the notes
          stay here too</label
        >
        <label class="choice"
          ><input type="radio" bind:group={moving} value={true} data-testid="transfer-move" /> Move: they leave
          this graph (its trash keeps them 30 days)</label
        >
      </fieldset>

      {#if req.kind === 'pages' && hasSubpages}
        <label class="choice"
          ><input type="checkbox" bind:checked={withSub} data-testid="transfer-subpages" /> Also the pages below
          it</label
        >
      {/if}
      {#if req.kind === 'blocks'}
        <label class="field">
          Into the page
          <input bind:value={targetPage} data-testid="transfer-page-name" />
        </label>
        <p class="muted">
          The page is made when the graph has none of that name; otherwise the blocks go to its end.
        </p>
      {/if}
      {#if showClash}
        <fieldset data-testid="transfer-clash">
          <legend>A page of that name exists</legend>
          <label class="choice"
            ><input type="radio" bind:group={clash} value="keep_both" /> Keep both (the copy is named “… (from
            {plan?.from_name})”)</label
          >
          <label class="choice"
            ><input type="radio" bind:group={clash} value="append" /> Add the blocks to the end of the existing
            page</label
          >
          <label class="choice"
            ><input type="radio" bind:group={clash} value="skip" /> Leave that page out</label
          >
        </fieldset>
      {/if}

      {#if plan}
        <div class="plan" data-testid="transfer-plan" aria-live="polite">
          <ul>
            {#each plan.items as i (i.source + i.target)}
              <li>
                {#if i.action === 'create'}New page{:else if i.action === 'append'}Added to{:else}Left out{/if}
                <strong>{i.target}</strong>
                {#if i.action === 'skip'}(it exists){/if}
              </li>
            {/each}
          </ul>
          {#if plan.assets.length}
            <p>
              {plan.assets.length} picture{plan.assets.length === 1 ? '' : 's'} or file{plan.assets.length ===
              1
                ? ''
                : 's'} go along{#if count('reuse')}; {count('reuse')} already there{/if}{#if count('rename')};
                {count('rename')} get{count('rename') === 1 ? 's' : ''} another name because the file differs{/if}{#if count('missing')};
                <span class="warn">{count('missing')} missing in {plan.from_name}</span>{/if}.
            </p>
          {/if}
          {#if plan.missing_links.length}
            <p>
              <span data-testid="transfer-links"
                >{plan.to_name} has no page for: {plan.missing_links.slice(0, 5).join(', ')}{plan
                  .missing_links.length > 5
                  ? '…'
                  : ''}.</span
              >
              The links stay and point to empty pages there.
            </p>
          {/if}
          {#if plan.dangling_refs}
            <p>
              {plan.dangling_refs} block reference{plan.dangling_refs === 1 ? '' : 's'} point{plan.dangling_refs ===
              1
                ? 's'
                : ''} at blocks that do not go along.
            </p>
          {/if}
          {#if plan.removes && plan.left_behind.length}
            <p data-testid="transfer-left-behind">
              Still linking to what leaves: {plan.left_behind.slice(0, 5).join(', ')}{plan.left_behind
                .length > 5
                ? '…'
                : ''}.
            </p>
          {/if}
          {#each plan.warnings as w (w)}<p class="warn" data-testid="transfer-warning">{w}</p>{/each}
        </div>
      {/if}
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="row">
      <button
        type="button"
        class="go"
        disabled={busy || !plan || !doing.length}
        onclick={go}
        data-testid="transfer-go">{moving ? 'Move' : 'Copy'}</button
      >
      <button type="button" class="plain" onclick={close}>Cancel</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    justify-content: center;
    padding-top: 8vh;
    z-index: 50;
  }
  .dialog {
    width: min(560px, 92vw);
    max-height: 84vh;
    overflow: auto;
    background: var(--panel);
    border-radius: 10px;
    padding: 1em;
    align-self: flex-start;
  }
  h2 {
    margin: 0 0 0.3em;
    font-size: 1.1em;
  }
  .muted {
    color: var(--muted);
    margin: 0 0 0.6em;
  }
  fieldset {
    border: 1px solid var(--border);
    border-radius: 6px;
    margin: 0 0 0.6em;
    padding: 0.3em 0.6em 0.5em;
  }
  legend {
    color: var(--muted);
    font-size: 0.85em;
    padding: 0 0.3em;
  }
  .choice {
    display: flex;
    align-items: baseline;
    gap: 0.5em;
    margin: 0.15em 0;
  }
  .graph {
    display: inline-flex;
    align-items: center;
    gap: 0.4em;
  }
  .gdot {
    width: 0.7em;
    height: 0.7em;
    border-radius: 50%;
    background: var(--graph-color, var(--muted));
  }
  .where {
    color: var(--muted);
    font-size: 0.8em;
    margin-left: auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 55%;
  }
  .field {
    display: block;
  }
  .field input {
    display: block;
    width: 100%;
    box-sizing: border-box;
    padding: 0.4em;
    margin-top: 0.2em;
    font: inherit;
  }
  .plan {
    border-top: 1px solid var(--border);
    margin-top: 0.4em;
    padding-top: 0.3em;
  }
  .plan ul {
    margin: 0.2em 0;
    padding-left: 1.2em;
  }
  .plan p {
    margin: 0.3em 0;
  }
  .warn {
    color: var(--warn);
  }
  .error {
    color: var(--danger);
  }
  .row {
    display: flex;
    gap: 0.5em;
    margin-top: 0.8em;
  }
  .go {
    font-weight: 600;
  }
</style>
