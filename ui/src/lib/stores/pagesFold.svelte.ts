// "Collapse all" / "Expand all" of the Pages part of the left panel. The tree and the property groups both listen:
// `n` counts the requests, `collapsed` says which one the last was.
export const pagesFold = $state({ n: 0, collapsed: true })

export function foldAll(collapsed: boolean) {
  pagesFold.collapsed = collapsed
  pagesFold.n++
}
