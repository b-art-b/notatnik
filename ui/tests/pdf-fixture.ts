// PDF files for the browser tests (PA-1..PA-10), made here so no binary has to be reviewed. Plain ASCII, one text line
// for each `Tj`, the standard font Helvetica (not embedded). `password` encrypts the file with the old RC4 40-bit
// scheme (revision 2) that pdf.js reads.
import crypto from 'node:crypto'

export interface PdfPageSpec {
  lines: string[]
  /** /Rotate */
  rotate?: 0 | 90 | 180 | 270
}

const PAD = Buffer.from('28bf4e5e4e758a4164004e56fffa01082e2e00b6d0683e802f0ca9fe6453697a', 'hex')

function rc4(key: Buffer, data: Buffer): Buffer {
  const s = Array.from({ length: 256 }, (_, i) => i)
  let j = 0
  for (let i = 0; i < 256; i++) {
    j = (j + s[i] + key[i % key.length]) & 255
    ;[s[i], s[j]] = [s[j], s[i]]
  }
  const out = Buffer.alloc(data.length)
  let a = 0
  let b = 0
  for (let k = 0; k < data.length; k++) {
    a = (a + 1) & 255
    b = (b + s[a]) & 255
    ;[s[a], s[b]] = [s[b], s[a]]
    out[k] = data[k] ^ s[(s[a] + s[b]) & 255]
  }
  return out
}

const md5 = (b: Buffer) => crypto.createHash('md5').update(b).digest()
const escapeText = (s: string) => s.replace(/[\\()]/g, '\\$&')

export function makePdf(pages: PdfPageSpec[], opts: { password?: string } = {}): Buffer {
  const objs: Buffer[] = [] // object number n is objs[n - 1]
  const nPages = pages.length
  // 1 catalog, 2 pages, 3 font, then (page, content) pairs; 4 + 2 * nPages: the encryption dictionary
  const pageObj = (i: number) => 4 + 2 * i
  const contentObj = (i: number) => 5 + 2 * i
  const encObj = 4 + 2 * nPages
  const id = crypto.createHash('md5').update('notatnik-fixture').digest()

  let key: Buffer | null = null
  let oEntry = Buffer.alloc(0)
  let uEntry = Buffer.alloc(0)
  const P = -4
  if (opts.password !== undefined) {
    const pw = Buffer.concat([Buffer.from(opts.password, 'latin1'), PAD]).subarray(0, 32)
    oEntry = rc4(md5(pw).subarray(0, 5), pw)
    const p = Buffer.alloc(4)
    p.writeInt32LE(P)
    key = md5(Buffer.concat([pw, oEntry, p, id])).subarray(0, 5)
    uEntry = rc4(key, PAD)
  }
  const objectKey = (n: number) =>
    md5(Buffer.concat([key!, Buffer.from([n & 255, (n >> 8) & 255, (n >> 16) & 255, 0, 0])])).subarray(
      0,
      Math.min(key!.length + 5, 16),
    )

  objs.push(Buffer.from('<< /Type /Catalog /Pages 2 0 R >>'))
  objs.push(
    Buffer.from(
      `<< /Type /Pages /Count ${nPages} /Kids [${pages.map((_, i) => `${pageObj(i)} 0 R`).join(' ')}] >>`,
    ),
  )
  objs.push(Buffer.from('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'))
  pages.forEach((pg, i) => {
    const rot = pg.rotate ? ` /Rotate ${pg.rotate}` : ''
    objs.push(
      Buffer.from(
        `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]${rot} /Resources << /Font << /F1 3 0 R >> >> /Contents ${contentObj(i)} 0 R >>`,
      ),
    )
    const stream = pg.lines
      .map((l, k) => `BT /F1 14 Tf 72 ${720 - k * 24} Td (${escapeText(l)}) Tj ET`)
      .join('\n')
    let data = Buffer.from(stream, 'latin1')
    if (key) data = rc4(objectKey(contentObj(i)), data)
    objs.push(
      Buffer.concat([
        Buffer.from(`<< /Length ${data.length} >>\nstream\n`),
        data,
        Buffer.from('\nendstream'),
      ]),
    )
  })
  if (key) {
    objs.push(
      Buffer.from(
        `<< /Filter /Standard /V 1 /R 2 /O <${oEntry.toString('hex')}> /U <${uEntry.toString('hex')}> /P ${P} >>`,
      ),
    )
  }
  const parts: Buffer[] = [Buffer.from('%PDF-1.4\n')]
  const offsets: number[] = []
  let len = parts[0].length
  objs.forEach((o, i) => {
    offsets.push(len)
    const b = Buffer.concat([Buffer.from(`${i + 1} 0 obj\n`), o, Buffer.from('\nendobj\n')])
    parts.push(b)
    len += b.length
  })
  const xref =
    `xref\n0 ${objs.length + 1}\n0000000000 65535 f \n` +
    offsets.map((o) => `${String(o).padStart(10, '0')} 00000 n \n`).join('')
  const trailer = `trailer\n<< /Size ${objs.length + 1} /Root 1 0 R${key ? ` /Encrypt ${encObj} 0 R` : ''} /ID [<${id.toString('hex')}> <${id.toString('hex')}>] >>\nstartxref\n${len}\n%%EOF\n`
  parts.push(Buffer.from(xref + trailer))
  return Buffer.concat(parts)
}

/** The text of the first fixture: two paragraphs on page 1, one line on page 2. */
export const REPORT: PdfPageSpec[] = [
  {
    lines: [
      'The mitochondrion is the powerhouse of the cell.',
      'Photosynthesis happens in the chloroplast.',
      'Zebrafish are a model organism.',
    ],
  },
  { lines: ['Second page: results and discussion.', 'Nothing else to see here.'] },
]
