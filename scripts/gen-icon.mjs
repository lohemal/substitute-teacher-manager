// 앱 아이콘 원본(1024x1024 PNG)을 생성한다.
// 외부 의존성 없이 zlib만 사용한다.
//   node scripts/gen-icon.mjs   ->  scripts/appicon.png
//   npx tauri icon scripts/appicon.png
import { deflateSync } from 'node:zlib'
import { writeFileSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const SIZE = 1024
const here = dirname(fileURLToPath(import.meta.url))

// ---- 도형 유틸 (SDF) ----------------------------------------------------

const clamp = (v, a, b) => (v < a ? a : v > b ? b : v)
const smoothstep = (e0, e1, x) => {
  const t = clamp((x - e0) / (e1 - e0), 0, 1)
  return t * t * (3 - 2 * t)
}

/** 중심 기준 둥근 사각형까지의 거리 */
function sdRoundRect(px, py, halfW, halfH, r) {
  const qx = Math.abs(px) - (halfW - r)
  const qy = Math.abs(py) - (halfH - r)
  const ax = Math.max(qx, 0)
  const ay = Math.max(qy, 0)
  return Math.hypot(ax, ay) + Math.min(Math.max(qx, qy), 0) - r
}

/** 선분까지의 거리 */
function sdSegment(px, py, ax, ay, bx, by) {
  const pax = px - ax
  const pay = py - ay
  const bax = bx - ax
  const bay = by - ay
  const h = clamp((pax * bax + pay * bay) / (bax * bax + bay * bay), 0, 1)
  return Math.hypot(pax - bax * h, pay - bay * h)
}

const mix = (a, b, t) => a + (b - a) * t

// ---- 픽셀 생성 ----------------------------------------------------------

// 하늘색 세로 그라데이션(아주 약하게) + 흰색 체크 + 연두색 포인트
const TOP = [56, 189, 248] // #38BDF8
const BOT = [14, 165, 233] // #0EA5E9
const LIME = [163, 230, 53] // #A3E635

const raw = Buffer.alloc(SIZE * (SIZE * 4 + 1))

for (let y = 0; y < SIZE; y++) {
  const rowStart = y * (SIZE * 4 + 1)
  raw[rowStart] = 0 // filter: none

  for (let x = 0; x < SIZE; x++) {
    const cx = x + 0.5 - SIZE / 2
    const cy = y + 0.5 - SIZE / 2

    // 타일: 여백 40px, 모서리 반경 224
    const dTile = sdRoundRect(cx, cy, SIZE / 2 - 40, SIZE / 2 - 40, 224)
    const tile = 1 - smoothstep(-1, 1, dTile)

    const t = y / (SIZE - 1)
    let r = mix(TOP[0], BOT[0], t)
    let g = mix(TOP[1], BOT[1], t)
    let b = mix(TOP[2], BOT[2], t)

    // 체크 표시 (흰색)
    const dCheck = Math.min(
      sdSegment(x + 0.5, y + 0.5, 300, 528, 452, 678),
      sdSegment(x + 0.5, y + 0.5, 452, 678, 728, 356),
    )
    const check = 1 - smoothstep(44, 46, dCheck)
    r = mix(r, 255, check)
    g = mix(g, 255, check)
    b = mix(b, 255, check)

    // 우상단 연두색 포인트 (Accent)
    const dDot = Math.hypot(x + 0.5 - 742, y + 0.5 - 292) - 46
    const dot = 1 - smoothstep(-1, 1, dDot)
    r = mix(r, LIME[0], dot)
    g = mix(g, LIME[1], dot)
    b = mix(b, LIME[2], dot)

    const i = rowStart + 1 + x * 4
    raw[i] = Math.round(r)
    raw[i + 1] = Math.round(g)
    raw[i + 2] = Math.round(b)
    raw[i + 3] = Math.round(tile * 255)
  }
}

// ---- PNG 인코딩 ---------------------------------------------------------

function crc32(buf) {
  let c
  const table = crc32.table ?? (crc32.table = (() => {
    const t = new Int32Array(256)
    for (let n = 0; n < 256; n++) {
      c = n
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
      t[n] = c
    }
    return t
  })())
  let crc = -1
  for (let i = 0; i < buf.length; i++) crc = (crc >>> 8) ^ table[(crc ^ buf[i]) & 0xff]
  return (crc ^ -1) >>> 0
}

function chunk(type, data) {
  const len = Buffer.alloc(4)
  len.writeUInt32BE(data.length)
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data])
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(body))
  return Buffer.concat([len, body, crc])
}

const ihdr = Buffer.alloc(13)
ihdr.writeUInt32BE(SIZE, 0)
ihdr.writeUInt32BE(SIZE, 4)
ihdr[8] = 8 // bit depth
ihdr[9] = 6 // RGBA
ihdr[10] = 0
ihdr[11] = 0
ihdr[12] = 0

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', ihdr),
  chunk('IDAT', deflateSync(raw, { level: 9 })),
  chunk('IEND', Buffer.alloc(0)),
])

mkdirSync(here, { recursive: true })
const out = join(here, 'appicon.png')
writeFileSync(out, png)
console.log(`생성 완료: ${out} (${SIZE}x${SIZE}, ${(png.length / 1024).toFixed(1)}KB)`)
