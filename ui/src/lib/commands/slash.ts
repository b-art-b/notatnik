// Slash commands in the editor (`/today`, `/todo`, ...). Dates come from the server's clock (injectable in tests).
import { app } from '../stores/app.svelte'
import { parseNaturalDate } from '../dates'

export interface SlashCommand {
  name: string
  title: string
  insert: () => { text: string; cursor?: number }
}

const link = (d: string) => `[[${d}]]`

export function slashCommands(): SlashCommand[] {
  const today = app.today || new Date().toISOString().slice(0, 10)
  const day = (phrase: string) => parseNaturalDate(phrase, today) ?? today
  return [
    { name: 'today', title: "Insert today's date", insert: () => ({ text: link(day('today')) }) },
    { name: 'tomorrow', title: "Insert tomorrow's date", insert: () => ({ text: link(day('tomorrow')) }) },
    { name: 'yesterday', title: "Insert yesterday's date", insert: () => ({ text: link(day('yesterday')) }) },
    {
      name: 'date',
      title: 'Insert a date (type e.g. next monday)',
      insert: () => ({ text: '[[]]', cursor: 2 }),
    },
    { name: 'todo', title: 'Make this a task', insert: () => ({ text: 'TODO ' }) },
    { name: 'doing', title: 'Mark as doing', insert: () => ({ text: 'DOING ' }) },
    { name: 'code', title: 'Code block', insert: () => ({ text: '```\n\n```', cursor: 4 }) },
    { name: 'query', title: 'Query block', insert: () => ({ text: '```query\ntask: todo\n```', cursor: 9 }) },
    { name: 'scheduled', title: 'Set scheduled date', insert: () => ({ text: `\nscheduled:: ${today}` }) },
    { name: 'deadline', title: 'Set deadline', insert: () => ({ text: `\ndeadline:: ${today}` }) },
    { name: 'priority', title: 'Set priority', insert: () => ({ text: '\npriority:: high' }) },
  ]
}
