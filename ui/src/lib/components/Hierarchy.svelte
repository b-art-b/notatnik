<script lang="ts">
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

  let { page }: { page: string } = $props()
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

{#if rows.length > 0}
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
  </section>
{/if}

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
  li.virtual a:last-child {
    font-style: italic;
  }
</style>
