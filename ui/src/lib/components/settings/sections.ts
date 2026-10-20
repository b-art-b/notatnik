// The sections of Settings, the groups they are listed in, and the words each one is found by in the search box.

export type SectionId =
  | 'appearance'
  | 'dates'
  | 'tasks'
  | 'icons'
  | 'templates'
  | 'sync'
  | 'calendar'
  | 'files'
  | 'backup'
  | 'graph'

export interface Section {
  id: SectionId
  label: string
  /** The path of a 24 x 24 line icon. */
  icon: string
  /** What the search box looks for besides the label. */
  words: string
}

export const SETTINGS_GROUPS: { label: string; items: Section[] }[] = [
  {
    label: 'Look and feel',
    items: [
      {
        id: 'appearance',
        label: 'Appearance',
        // Lucide-style half circle (theme)
        icon: 'M12 3a9 9 0 1 0 0 18V3z M12 3a9 9 0 0 1 0 18',
        words:
          'theme dark light zoom sidebar width brackets links references side panel task details properties compact',
      },
      {
        id: 'dates',
        label: 'Dates & language',
        icon: 'M5 5h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z M8 3v4 M16 3v4 M3 10h18',
        words: 'date format locale language week starts monday sunday iso pattern',
      },
    ],
  },
  {
    label: 'Notes',
    items: [
      {
        id: 'tasks',
        label: 'Tasks',
        icon: 'M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z M8 12.5l3 3 5-6',
        words: 'tasks priority priorities completion log repeat deadline todo',
      },
      {
        id: 'icons',
        label: 'Page icons',
        icon: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z M8.5 14a4 4 0 0 0 7 0 M9 9.5h.01 M15 9.5h.01',
        words: 'icons emoji page type rules',
      },
      {
        id: 'templates',
        label: 'Templates',
        icon: 'M8 4h10a2 2 0 0 1 2 2v12 M4 8a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z',
        words: 'templates placeholders journal new page namespace type automatic',
      },
    ],
  },
  {
    label: 'Servers',
    items: [
      {
        id: 'sync',
        label: 'Sync',
        icon: 'M7 18a4 4 0 0 1-.6-7.96A6 6 0 0 1 18 9.5a4.5 4.5 0 0 1-.5 8.5H7z',
        words:
          'sync nextcloud webdav server connect upload encryption encrypt password recovery key lock keyring disconnect merge',
      },
      {
        id: 'calendar',
        label: 'Calendar',
        icon: 'M5 5h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z M8 3v4 M16 3v4 M3 10h18 M8 14h3',
        words: 'calendar caldav tasks vtodo events davx nextcloud login',
      },
      {
        id: 'files',
        label: 'Linked files',
        icon: 'M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1 M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1',
        words: 'linked files file source server link nextcloud picker',
      },
    ],
  },
  {
    label: 'Data',
    items: [
      {
        id: 'backup',
        label: 'Backup',
        icon: 'M12 3v12 M7 10l5 5 5-5 M4 19h16',
        words:
          'backup snapshot restore encryption encrypt password recovery key compression second folder mirror automatic',
      },
      {
        id: 'graph',
        label: 'Graph',
        icon: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z',
        words: 'graph folder switch open last start unused images assets',
      },
    ],
  },
]

export const SECTIONS: Section[] = SETTINGS_GROUPS.flatMap((g) => g.items)

/** The sections whose name or words contain every word of `query` (all of them for an empty query). */
export function sectionsMatching(query: string): Set<SectionId> {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean)
  return new Set(
    SECTIONS.filter((s) => {
      const text = `${s.label} ${s.words}`.toLowerCase()
      return terms.every((t) => text.includes(t))
    }).map((s) => s.id),
  )
}
