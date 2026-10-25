// The colors a graph can have (MG-5). The palette itself is in `app.css` (`--graph-0` ... `--graph-7`); the backend
// stores only the index, and its default comes from the graph's ID. The name is always shown beside a color: a color
// alone tells nothing to someone who cannot tell them apart.
export const GRAPH_COLOR_NAMES = ['Blue', 'Green', 'Orange', 'Purple', 'Pink', 'Teal', 'Red', 'Gold'] as const

/** The CSS color of palette entry `index`. */
export const graphColorVar = (index: number) => `var(--graph-${index % GRAPH_COLOR_NAMES.length})`

export const graphColorName = (index: number) => GRAPH_COLOR_NAMES[index % GRAPH_COLOR_NAMES.length]
