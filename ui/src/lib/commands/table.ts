// The command table: one list of (id, title, keys, action) used by the palette, global shortcuts and docs/keymap.md.
import {
  app,
  applyTheme,
  navigate,
  toast,
  toggleLeft,
  toggleRight,
  toggleRightFocus,
} from '../stores/app.svelte'
import { openDocs } from '../stores/pages.svelte'
import { globalUndo } from './exec'
import { syncNow } from '../stores/sync.svelte'
import { calendarSyncNow } from '../calendar'

export interface Command {
  id: string
  title: string
  keys: string
  /** Handled inside the editor/selection code, listed here so the palette can show it. */
  editorOnly?: boolean
  run?: () => void
}

export const commands: Command[] = [
  { id: 'palette', title: 'Command palette', keys: 'Ctrl+K', run: () => (app.paletteOpen = true) },
  { id: 'search', title: 'Search', keys: 'Ctrl+Shift+F', run: () => (app.searchOpen = true) },
  {
    id: 'switch-graph',
    title: 'Switch graph…',
    keys: '',
    run: () => (app.needsGraph = true),
  },
  {
    id: 'sync-now',
    title: 'Sync now',
    keys: '',
    run: () => {
      if (app.graph?.remote) void syncNow()
      else toast('This graph is not kept on a server.', 'info')
    },
  },
  {
    id: 'calendar-sync',
    title: 'Sync tasks to calendar',
    keys: '',
    run: () => void calendarSyncNow(),
  },
  { id: 'journals', title: 'Go to journals', keys: '', run: () => navigate({ kind: 'journals' }) },
  { id: 'all-pages', title: 'Go to all pages', keys: '', run: () => navigate({ kind: 'pages' }) },
  { id: 'settings', title: 'Settings', keys: '', run: () => navigate({ kind: 'settings' }) },
  { id: 'toggle-sidebar', title: 'Show or hide the sidebar', keys: 'Ctrl+\\', run: toggleLeft },
  { id: 'toggle-side-panel', title: 'Show or hide the side panel', keys: 'Ctrl+Shift+\\', run: toggleRight },
  {
    id: 'focus-side-panel',
    title: 'Focus on the side panel (fold the page away) or show the page again',
    keys: '',
    run: toggleRightFocus,
  },
  { id: 'undo', title: 'Undo', keys: 'Ctrl+Z', run: () => void globalUndo('undo') },
  { id: 'redo', title: 'Redo', keys: 'Ctrl+Shift+Z', run: () => void globalUndo('redo') },
  { id: 'theme-light', title: 'Theme: light', keys: '', run: () => applyTheme('light') },
  { id: 'theme-dark', title: 'Theme: dark', keys: '', run: () => applyTheme('dark') },
  { id: 'theme-system', title: 'Theme: follow system', keys: '', run: () => applyTheme('system') },
  {
    id: 'collapse-all',
    title: 'Collapse all blocks',
    keys: '',
    run: () => openDocs().forEach((d) => void d.setAllCollapsed(true)),
  },
  {
    id: 'expand-all',
    title: 'Expand all blocks',
    keys: '',
    run: () => openDocs().forEach((d) => void d.setAllCollapsed(false)),
  },
  { id: 'split-block', title: 'Split block / new block', keys: 'Enter', editorOnly: true },
  { id: 'newline', title: 'Newline inside block', keys: 'Shift+Enter', editorOnly: true },
  { id: 'indent', title: 'Indent block', keys: 'Tab / Alt+→', editorOnly: true },
  { id: 'outdent', title: 'Outdent block', keys: 'Shift+Tab / Alt+←', editorOnly: true },
  { id: 'move-up', title: 'Move block up', keys: 'Alt+↑', editorOnly: true },
  { id: 'move-down', title: 'Move block down', keys: 'Alt+↓', editorOnly: true },
  { id: 'nav-up', title: 'Previous block', keys: '↑ (first line)', editorOnly: true },
  { id: 'nav-down', title: 'Next block', keys: '↓ (last line)', editorOnly: true },
  { id: 'merge-prev', title: 'Merge with previous block', keys: 'Backspace (start)', editorOnly: true },
  { id: 'merge-next', title: 'Merge next block into this', keys: 'Delete (end)', editorOnly: true },
  { id: 'escape', title: 'Leave editing / select block', keys: 'Esc', editorOnly: true },
  { id: 'cycle-task', title: 'Cycle task state', keys: 'Ctrl+Enter', editorOnly: true },
]
