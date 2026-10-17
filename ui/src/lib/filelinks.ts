// Markdown links to files: from a server, from this computer, or any address typed in.

/** `[name](url)`; brackets in the name are escaped so they do not end the link text early. */
export function fileLink(name: string, url: string): string {
  const label = name.replace(/[[\]\\]/g, (c) => `\\${c}`)
  // a space or a parenthesis in the address would end it early; `<…>` is Markdown's way to write one as it is
  const target = /[\s()<>]/.test(url) ? `<${url.replace(/[<>]/g, encodeURIComponent)}>` : url
  return `[${label}](${target})`
}

/** `file:///home/me/My%20Docs/a.pdf` for an absolute path of this computer (Linux/macOS or Windows). */
export function fileUrl(path: string): string {
  const p = path.replace(/\\/g, '/')
  const encoded = p
    .split('/')
    .map((seg) => encodeURIComponent(seg).replace(/%3A$/i, ':'))
    .join('/')
  return encoded.startsWith('/') ? `file://${encoded}` : `file:///${encoded}`
}

/** The last part of a path or address, decoded: `a.pdf` for `/x/a.pdf` or `https://h/x/a%20b.pdf?dl=1`. */
export function lastName(s: string): string {
  const clean = s.split(/[?#]/)[0].replace(/[/\\]+$/, '')
  const tail = clean.split(/[/\\]/).pop() ?? ''
  try {
    return decodeURIComponent(tail)
  } catch {
    return tail
  }
}

/** The name a pasted address gets as link text: its file name, else its host. */
export function nameForAddress(address: string): string {
  const a = address.trim()
  const tail = lastName(a)
  if (tail && !/^[a-z][a-z0-9+.-]*:$/i.test(tail) && !a.endsWith(`//${tail}`)) return tail
  try {
    return new URL(a).host || a
  } catch {
    return a
  }
}

/** Only addresses the app opens: http, https, mailto and file. */
export function isLinkable(address: string): boolean {
  return /^(https?:\/\/[^\s/]+|mailto:[^\s]+|file:\/\/\/[^\s]+)/i.test(address.trim())
}

export function humanSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10240 ? 1 : 0)} KB`
  if (bytes < 1024 ** 3) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`
}
