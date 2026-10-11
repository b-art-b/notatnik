<script lang="ts">
  // The `key:: value` lines of a block as a card (PR-6): bold key, value with links, edit and remove on hover,
  // and a "+ property" row. Edits go through `setPropertyOf`, so only that one line of the file changes.
  import type { BlockView } from '../api'
  import { backend } from '../api'
  import { getDoc } from '../stores/pages.svelte'
  import { setPropertyOf } from '../commands/exec'
  import { blockPropertyHash, pageHash } from '../route'
  import { shownProps } from '../props'
  import ValueInput from './ValueInput.svelte'

  interface Props {
    block: BlockView
    docName: string
    readonly?: boolean
  }
  let { block, docName, readonly = false }: Props = $props()

  // Task data has its own chips (scheduled, deadline, priority, repeat); system properties are hidden.
  const shown = $derived(shownProps(block))

  // `$state` for what the user is typing; `editKey` is the property being edited, `adding` the new-property row
  let editKey = $state<string | null>(null)
  let draft = $state('')
  let adding = $state(false)
  let newKey = $state('')
  let newValue = $state('')
  let knownKeys = $state<string[]>([])
  let knownValues = $state<string[]>([])

  /** A Svelte *action*: runs once when the element is mounted. Here: focus the input as it appears. */
  function focus(node: HTMLInputElement) {
    node.focus()
    node.select()
  }

  // no glyph for link values: they are coloured as links already, and an arrow reads as a "source" marker
  const GLYPH: Record<string, string> = {
    date: '◷',
    datetime: '◷',
    number: '#',
    bool: '✓',
    list: '≡',
  }

  function begin(key: string, raw: string) {
    adding = false
    editKey = key
    draft = raw
  }

  async function save() {
    const key = editKey
    const value = draft.trim()
    editKey = null
    const doc = getDoc(docName)
    if (key && value && doc) await setPropertyOf(doc, block.id, key, value)
  }

  /** Values already used for a property, offered in its value field. */
  async function valuesOfKey(key: string): Promise<string[]> {
    return (await backend.call('property_values', { key })).items
  }

  async function remove(key: string) {
    editKey = null
    const doc = getDoc(docName)
    if (doc) await setPropertyOf(doc, block.id, key, null)
  }

  async function openAdd() {
    editKey = null
    adding = true
    newKey = ''
    newValue = ''
    try {
      knownKeys = (await backend.call('property_keys', {})).items
    } catch {
      knownKeys = []
    }
  }

  // values already used for the key being added, suggested in the value input
  $effect(() => {
    const k = newKey.trim()
    if (!adding || !k) {
      knownValues = []
      return
    }
    let cancelled = false
    backend
      .call('property_values', { key: k })
      .then((r) => {
        if (!cancelled) knownValues = r.items
      })
      .catch(() => {})
    return () => {
      cancelled = true
    }
  })

  const cleanKey = (s: string) =>
    s
      .trim()
      .toLowerCase()
      .replace(/[\s_]+/g, '-')
      .replace(/[^a-z0-9-]/g, '')

  async function create() {
    const key = cleanKey(newKey)
    const value = newValue.trim()
    adding = false
    const doc = getDoc(docName)
    if (key && value && doc) await setPropertyOf(doc, block.id, key, value)
  }

  /** A text value links to the view of its blocks, unless it has characters the query language would split. */
  const valueQueryable = (raw: string) => !/[,[\]]/.test(raw) && !/^[<>=!~]/.test(raw)
</script>

{#if shown.length || adding}
  <div class="card" data-testid="props">
    {#each shown as p, i (p.key)}
      <div class="prop" data-key={p.key}>
        {#if editKey === p.key}
          <form
            class="edit"
            onsubmit={(e) => {
              e.preventDefault()
              void save()
            }}
          >
            <span class="key">{p.key}:</span>
            <ValueInput
              focusOnMount
              bind:value={draft}
              label="Value of {p.key}"
              loadValues={() => valuesOfKey(p.key)}
              oncommit={() => void save()}
              onescape={() => (editKey = null)}
            />
            <button type="submit" class="ok" aria-label="Save {p.key}">✓</button>
          </form>
        {:else}
          {#if GLYPH[p.kind]}<span class="glyph" aria-hidden="true">{GLYPH[p.kind]}</span>{/if}
          <a class="key" href={blockPropertyHash(p.key)} title="All blocks with {p.key}">{p.key}:</a>
          <span class="val">
            {#if p.links.length}
              {#each p.links as l, i (i)}<a class="page-ref" href={pageHash(l)} data-page={l}>{l}</a
                >{#if i < p.links.length - 1},
                {/if}{/each}
            {:else if valueQueryable(p.raw)}
              <a class="plain" href={blockPropertyHash(p.key, p.raw)} title="All blocks with {p.key}: {p.raw}"
                >{p.raw}</a
              >
            {:else}{p.raw}{/if}
          </span>
          {#if !readonly}
            <span class="tools">
              <button class="tool" aria-label="Edit {p.key}" title="Edit" onclick={() => begin(p.key, p.raw)}
                >✎</button
              >
              <button class="tool" aria-label="Remove {p.key}" title="Remove" onclick={() => remove(p.key)}
                >×</button
              >
              {#if i === shown.length - 1 && !adding}
                <!-- the add button lives in the last row's tools: it takes no row of its own -->
                <button class="tool" aria-label="Add property" title="Add a property" onclick={openAdd}
                  >+</button
                >
              {/if}
            </span>
          {/if}
        {/if}
      </div>
    {/each}
    {#if !readonly}
      {#if adding}
        <form
          class="edit add"
          onsubmit={(e) => {
            e.preventDefault()
            void create()
          }}
        >
          <input
            use:focus
            bind:value={newKey}
            list="prop-keys-{block.id}"
            placeholder="property"
            aria-label="New property name"
            onkeydown={(e) => {
              if (e.key === 'Escape') {
                e.stopPropagation()
                adding = false
              }
            }}
          />
          <datalist id="prop-keys-{block.id}">
            {#each knownKeys as k (k)}<option value={k}></option>{/each}
          </datalist>
          <span class="key">::</span>
          <ValueInput
            bind:value={newValue}
            values={knownValues}
            placeholder="value or a page"
            label="New property value"
            onescape={() => (adding = false)}
          />
          <button type="submit" class="ok" aria-label="Add property">✓</button>
        </form>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .card {
    margin: 0.25em 0;
    padding: 0.3em 0.7em;
    border-radius: 6px;
    background: var(--chip);
    font-size: 0.85em;
    max-width: 100%;
  }
  .prop {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 0.4em;
    line-height: 1.6;
  }
  .glyph {
    width: 1.1em;
    text-align: center;
    color: var(--muted);
    font-size: 0.9em;
  }
  .key {
    font-weight: 700;
    color: var(--muted);
    text-decoration: none;
  }
  a.key:hover {
    text-decoration: underline;
  }
  .plain {
    color: inherit;
    text-decoration: none;
  }
  .plain:hover {
    text-decoration: underline;
  }
  .tools {
    margin-left: auto;
    display: inline-flex;
    gap: 0.2em;
    opacity: 0;
  }
  .prop:hover .tools,
  .prop:focus-within .tools {
    opacity: 1;
  }
  .tool,
  .ok {
    border: none;
    background: none;
    cursor: pointer;
    color: var(--muted);
    font: inherit;
    padding: 0 0.3em;
    border-radius: 4px;
  }
  .tool:hover,
  .ok:hover {
    background: var(--selection);
    color: var(--text);
  }
  .edit {
    display: flex;
    gap: 0.4em;
    align-items: baseline;
    flex: 1;
  }
  .edit input {
    flex: 1;
    min-width: 6em;
    font: inherit;
    padding: 0.1em 0.4em;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg, transparent);
    color: var(--text);
  }
</style>
