<script lang="ts">
  import Padlock from './Padlock.svelte'
  import type { PadlockMark } from '../stores/encryption.svelte'
  // "Hierarchy", as in Logseq: every page below this one (at any depth), each as its path with every segment a link.
  // The segments are normal page links, so the "brackets around links" setting applies to them as well.
  //
  // A segment that occurs in more than one place (`Renewal` in P/Capgemini/Renewal, P/EY/Renewal and X/Renewal) is a
  // topic rather than a location: it carries a small count and opens the view of all its places instead of one page.
  // That holds for every segment of a row, also the path of this page itself. Unique segments open their page.
  import { backend, type TitleHit } from '../api'
  import { bus } from '../stores/events.svelte'
  import { pageHash, segmentHash } from '../route'
  import PageIcon from './PageIcon.svelte'
  import { app } from '../stores/app.svelte'
  import { openPage } from '../newPage'

  // `lock`: the padlock new subpages get (see `padlockOf`): none when they follow the graph's default
  let { page, lock = null }: { page: string; lock?: PadlockMark | null } = $props()
  let adding = $state(false)
  let name = $state('')
  let problem = $state('')
  let field = $state<HTMLInputElement>()

  function startAdding() {
    name = `${page}/`
    problem = ''
    adding = true
  }
  // the palette command "New subpage of this page" asks for the field through `app.subpageFor`
  $effect(() => {
    if (app.subpageFor && app.subpageFor === page) {
      app.subpageFor = ''
      startAdding()
    }
  })
  // `bind:this` fills `field` once the input exists; focus it with the cursor at the end, after the slash
  $effect(() => {
    if (adding && field) {
      field.focus()
      field.setSelectionRange(name.length, name.length)
    }
  })
  async function submit() {
    // a name that is only the prefix is nothing yet
    if (name.trim() === `${page}/`) return void (problem = 'Type a name')
    const err = await openPage(name)
    if (err) problem = err
    else adding = false
  }
  let rows = $state<TitleHit[]>([])
  let places = $state<Record<string, number>>({}) // lowercased segment -> in how many places it occurs

  $effect(() => {
    void bus.version
    const name = page
    let cancelled = false
    ;(async () => {
      const r = await backend.call('subpages', { page: name })
      // every segment shown, once each
      const all = [...new Set(r.flatMap((x) => x.name.split('/')))]
      const counts = all.length ? (await backend.call('segment_places', { segments: all })).places : []
      if (cancelled) return
      rows = r
      places = Object.fromEntries(all.map((s, i) => [s.toLowerCase(), counts[i] ?? 0]))
    })()
    return () => {
      cancelled = true
    }
  })

  /** `Z/Partner Beta/Renewal` → Z, Z/Partner Beta, Z/Partner Beta/Renewal (what each segment links to). */
  function segments(name: string) {
    const parts = name.split('/')
    return parts.map((label, i) => ({
      label,
      target: parts.slice(0, i + 1).join('/'),
      n: places[label.toLowerCase()] ?? 0,
    }))
  }
</script>

<section class="hierarchy" aria-label="Hierarchy" data-testid="hierarchy">
  <h2>Hierarchy</h2>
  <ul>
    {#each rows as r (r.name)}
      <li data-page={r.name} class:virtual={r.is_virtual}>
        {#each segments(r.name) as s, i (s.target)}{#if i > 0}<span class="sep">/</span>{/if}{#if s.n > 1}<a
              class="page-ref topic"
              href={segmentHash(s.label)}
              data-segment={s.label}
              title={`“${s.label}” occurs in ${s.n} places: show them all`}
              >{s.label}<sup class="badge" data-testid="segment-badge">{s.n}</sup></a
            >{:else}<a class="page-ref" href={pageHash(s.target)} data-page={s.target}
              ><PageIcon name={s.target} />{s.label}</a
            >{/if}{/each}
      </li>
    {/each}
  </ul>
  {#if adding}
    <form
      class="add-sub"
      onsubmit={(e) => {
        e.preventDefault()
        void submit()
      }}
    >
      <input
        bind:this={field}
        bind:value={name}
        aria-label="Name of the new subpage"
        spellcheck="false"
        oninput={() => (problem = '')}
        onkeydown={(e) => {
          if (e.key === 'Escape') adding = false
        }}
      />
      <Padlock mark={lock} testid="subpage-lock" gap="0.4em" />
      {#if problem}<span class="problem" role="alert">{problem}</span>{/if}
    </form>
  {:else}
    <button class="add-sub-btn" onclick={startAdding}>+ Add subpage</button>
  {/if}
</section>

<style>
  .hierarchy {
    margin-top: 2em;
  }
  h2 {
    font-size: 0.9em;
    color: var(--muted);
    margin: 0 0 0.5em;
  }
  ul {
    margin: 0;
    padding-left: 1.4em;
    list-style: circle;
  }
  li {
    padding: 0.15em 0;
  }
  .sep {
    color: var(--muted);
    margin: 0 0.45em;
  }
  .badge {
    margin-left: 0.15em;
    font-size: 0.65em;
    color: var(--muted);
  }
  .add-sub-btn {
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 0.2em 0;
    margin-top: 0.3em;
    font: inherit;
  }
  .add-sub-btn:hover,
  .add-sub-btn:focus-visible {
    color: var(--accent);
  }
  .add-sub {
    display: flex;
    align-items: center;
    gap: 0.5em;
    margin-top: 0.3em;
  }
  .add-sub input {
    flex: 1;
    min-width: 0;
    font: inherit;
    padding: 0.2em 0.4em;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: inherit;
  }
  .problem {
    color: var(--danger);
    font-size: 0.85em;
  }
  li.virtual a:last-child {
    font-style: italic;
  }
</style>
