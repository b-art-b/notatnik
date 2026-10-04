# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: queries.spec.ts >> autocomplete >> property keys and values after `::`
- Location: tests/queries.spec.ts:141:3

# Error details

```
Error: expect(locator).toContainText(expected) failed

Locator: locator('.cm-tooltip-autocomplete')
Expected substring: "draft"
Timeout: 5000ms
Error: element(s) not found

Call log:
  - Expect "toContainText" locator('.cm-tooltip-autocomplete') with timeout 5000ms
  - waiting for locator('.cm-tooltip-autocomplete')

```

```yaml
- complementary "Sidebar":
  - navigation:
    - link "Journals":
      - /url: "#/journals"
    - link "Agenda":
      - /url: "#/agenda"
    - button "Search"
    - link "Settings":
      - /url: "#/settings"
  - status: ✓ saved
  - heading "Pages" [level=3]
  - tree "Namespaces":
    - treeitem "p":
      - link "p":
        - /url: "#/page/p"
    - treeitem "Expand people 1":
      - button "Expand": ▸
      - link "people":
        - /url: "#/page/people"
      - text: "1"
    - treeitem "project":
      - link "project":
        - /url: "#/page/project"
    - treeitem "Expand reading 1":
      - button "Expand": ▸
      - link "reading":
        - /url: "#/page/reading"
      - text: "1"
    - treeitem "someday":
      - link "someday":
        - /url: "#/page/someday"
    - treeitem "urgent":
      - link "urgent":
        - /url: "#/page/urgent"
    - treeitem "Expand work 4":
      - button "Expand": ▸
      - link "work":
        - /url: "#/page/work"
      - text: "4"
    - treeitem "Expand workshop 1":
      - button "Expand": ▸
      - link "workshop":
        - /url: "#/page/workshop"
      - text: "1"
- main:
  - article "p":
    - navigation "Namespace"
    - heading "p" [level=1]:
      - button "p"
    - tree "Blocks":
      - 'treeitem "Zoom into block A blockrating:: 3sta"':
        - button "Zoom into block": ●
        - textbox "Block text"
    - button "Add block": +
    - region "Linked references":
      - button "▾ Linked references 0" [expanded]
```

# Test source

```ts
  52  |     expect(lines.filter((l) => l.includes('finished')).length).toBe(1)
  53  |   })
  54  | 
  55  |   test("kanban: dragging a card changes only that task's state", async ({ backend, page }) => {
  56  |     backend.write(
  57  |       'pages/board.md',
  58  |       PAGE('task: todo, doing\nns: work/projects/alpha\ngroup: task\nview: kanban'),
  59  |     )
  60  |     await backend.open('/page/board')
  61  |     const before = backend.read('pages/work/projects/alpha.md')
  62  |     await expect(page.getByTestId('query-kanban')).toBeVisible()
  63  |     const card = page.locator('[data-card]', { hasText: 'Write spec' })
  64  |     await card.dragTo(page.locator('[data-col="doing"]'))
  65  |     await expect
  66  |       .poll(() => backend.read('pages/work/projects/alpha.md'))
  67  |       .toContain('- DOING Write spec #urgent')
  68  |     const after = backend.read('pages/work/projects/alpha.md')
  69  |     expect(after.replace('DOING Write spec', 'TODO Write spec')).toBe(before)
  70  |   })
  71  | 
  72  |   test('kanban by property: dragging changes the property line', async ({ backend, page }) => {
  73  |     backend.write('pages/board2.md', PAGE('page: reading/books\ngroup: prop.status\nview: kanban'))
  74  |     await backend.open('/page/board2')
  75  |     await page.locator('[data-card]', { hasText: 'Solaris' }).dragTo(page.locator('[data-col="done"]'))
  76  |     await expect.poll(() => backend.read('pages/reading/books.md')).toContain('rating:: 4\n  status:: done')
  77  |   })
  78  | 
  79  |   test('timeline: tasks sit on their deadline; dragging moves the date by the dragged days', async ({
  80  |     backend,
  81  |     page,
  82  |   }) => {
  83  |     backend.write('pages/cal.md', PAGE('ns: work/projects/alpha\ntask: any\nview: timeline'))
  84  |     await backend.open('/page/cal')
  85  |     const ev = page.locator('[data-ev]', { hasText: 'Review design' })
  86  |     await expect(ev).toBeVisible()
  87  |     await expect(page.locator('td[data-date="2026-10-09"] [data-ev]')).toContainText('Review design')
  88  |     await ev.dragTo(page.locator('td[data-date="2026-10-12"]'))
  89  |     await expect.poll(() => backend.read('pages/work/projects/alpha.md')).toContain('deadline:: 2026-10-12')
  90  |   })
  91  | 
  92  |   test('view switcher, show query and save to sidebar', async ({ backend, page }) => {
  93  |     backend.write('pages/dash.md', PAGE('task: todo\nns: work'))
  94  |     await backend.open('/page/dash')
  95  |     await page.getByRole('button', { name: 'table', exact: true }).click()
  96  |     await expect(page.getByTestId('query-table')).toBeVisible()
  97  |     await page.getByRole('button', { name: 'show query' }).click()
  98  |     await expect(page.locator('pre.code')).toContainText('ns: work')
  99  |     await page.getByRole('button', { name: 'save', exact: true }).click()
  100 |     await page.getByRole('textbox', { name: 'Name' }).fill('Open work')
  101 |     await page.getByRole('button', { name: 'Save to sidebar' }).click()
  102 |     await expect(page.getByRole('link', { name: 'Open work' })).toBeVisible()
  103 |     expect(backend.read('.notatnik/config.toml')).toContain('Open work')
  104 |     await page.getByRole('link', { name: 'Open work' }).click()
  105 |     await expect(page.getByTestId('query-count')).toHaveText('5 results')
  106 |   })
  107 | 
  108 |   test('list view: editing a block inside results writes to the source page', async ({ backend, page }) => {
  109 |     backend.write('pages/dash.md', PAGE('page: work/tasks\ntask: todo'))
  110 |     await backend.open('/page/dash')
  111 |     await page.locator('[data-testid="query-list"]').getByText('Work root task').click()
  112 |     await expect(page.getByTestId('block-editor')).toBeFocused()
  113 |     await page.keyboard.press('End')
  114 |     await page.keyboard.type(' edited')
  115 |     await page.keyboard.press('Escape')
  116 |     await expect.poll(() => backend.read('pages/work/tasks.md')).toContain('- TODO Work root task edited')
  117 |   })
  118 | })
  119 | 
  120 | test.describe('autocomplete', () => {
  121 |   test.use({ fixture: 'queries' })
  122 | 
  123 |   test('clause keys inside a query block', async ({ backend, page }) => {
  124 |     backend.write('pages/q.md', '- ```query\n  task: todo\n  ```\n')
  125 |     await backend.open('/page/q')
  126 |     await page
  127 |       .getByTestId('query-block')
  128 |       .locator('xpath=ancestor::div[contains(@class,"body")]')
  129 |       .click({ position: { x: 3, y: 3 } })
  130 |     await expect(page.getByTestId('block-editor')).toBeFocused()
  131 |     await page.keyboard.press('Control+End')
  132 |     await page.keyboard.press('ArrowUp')
  133 |     await page.keyboard.press('End')
  134 |     await page.keyboard.press('Enter')
  135 |     await page.keyboard.type('ta')
  136 |     const tip = page.locator('.cm-tooltip-autocomplete')
  137 |     await expect(tip).toContainText('task', { timeout: 5000 })
  138 |     await expect(tip).toContainText('tag')
  139 |   })
  140 | 
  141 |   test('property keys and values after `::`', async ({ backend, page }) => {
  142 |     backend.write('pages/p.md', '- A block\n  rating:: 3\n')
  143 |     await backend.open('/page/p')
  144 |     await page.getByText('A block').click()
  145 |     await expect(page.getByTestId('block-editor')).toBeFocused()
  146 |     await page.keyboard.press('Control+End')
  147 |     await page.keyboard.press('Shift+Enter')
  148 |     await page.keyboard.type('sta')
  149 |     await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('status', { timeout: 5000 })
  150 |     await expect(page.locator('.cm-tooltip-autocomplete [aria-selected="true"]')).toBeVisible()
  151 |     await page.keyboard.press('Enter')
> 152 |     await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('draft', { timeout: 5000 })
      |                                                            ^ Error: expect(locator).toContainText(expected) failed
  153 |     await expect(page.locator('.cm-tooltip-autocomplete')).toContainText('reading')
  154 |   })
  155 | })
  156 | 
```