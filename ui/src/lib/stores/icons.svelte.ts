// Page icons of the active graph (`IconsStore`); `iconOf` can be called from any template.
import { active } from './active.svelte'

export const icons = {
  get map() {
    return active().icons.map
  },
}

export const loadIcons = () => active().icons.load()

/** Whether any page of the graph has an icon: a list then keeps room for it on rows that have none (`PageIcon align`). */
export const anyIcons = () => active().icons.any()

/** The icon of a page, or an empty string. */
export const iconOf = (name: string) => active().icons.of(name)
