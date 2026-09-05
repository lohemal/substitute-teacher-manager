import type { BulkTeacherRow, RoleCode } from '@/ipc/teacher'

/**
 * 엑셀·한글 표에서 복사한 교사 명단을 읽는다.
 *
 * 칸 구분은 탭(엑셀 복사) 또는 쉼표(CSV)를 모두 받는다.
 * 열 순서는 정해두지 않고 내용을 보고 알아서 판단한다.
 *
 *   김민수	담임	1-1
 *   이영희	담임	1학년 2반
 *   박지훈	전담	체육, 음악
 *   최수정	기타	보건교사
 *   강호준            <- 이름만 있어도 된다
 */

export interface ParsedRow extends BulkTeacherRow {
  /** 화면 미리보기에 쓰는 원본 줄 */
  raw: string
  note?: string
}

interface ClassCell {
  grade: number
  classNo?: number
  className?: string
}

const ROLE_WORDS: { words: string[]; code: RoleCode }[] = [
  { words: ['담임'], code: 'HOMEROOM' },
  { words: ['전담', '교과'], code: 'SPECIAL' },
  { words: ['기타', '교장', '교감', '보건', '상담', '특수', '사서', '영양', '행정'], code: 'OTHER' },
]

/**
 * 학급 칸을 읽는다. 반 번호와 반 이름을 모두 받는다.
 *
 *   1-1 / 1학년 2반 / 5 - 3 / 6-2반        -> 반 번호
 *   1-가람 / 1학년 가람반 / 1-가람반         -> 반 이름
 */
function parseClass(cell: string): ClassCell | null {
  const t = cell.replace(/\s/g, '')

  // 반 번호
  let m = /^(\d{1,2})[-–~](\d{1,2})반?$/.exec(t)
  if (m) return { grade: Number(m[1]), classNo: Number(m[2]) }
  m = /^(\d{1,2})학년(\d{1,2})반$/.exec(t)
  if (m) return { grade: Number(m[1]), classNo: Number(m[2]) }

  // 반 이름 ('가람', '가람반')
  m = /^(\d{1,2})[-–~]([^\d\s]{1,10}?)반?$/.exec(t)
  if (m) return { grade: Number(m[1]), className: m[2] }
  m = /^(\d{1,2})학년([^\d\s]{1,10}?)반$/.exec(t)
  if (m) return { grade: Number(m[1]), className: m[2] }

  return null
}

function parseRole(cell: string): RoleCode | null {
  const t = cell.replace(/\s/g, '')
  for (const { words, code } of ROLE_WORDS) {
    if (words.some((w) => t === w || t.includes(w))) return code
  }
  return null
}

function splitCells(line: string): string[] {
  const raw = line.includes('\t') ? line.split('\t') : line.split(',')
  return raw.map((c) => c.trim())
}

export function parseRoster(text: string): ParsedRow[] {
  const out: ParsedRow[] = []

  for (const line of text.split(/\r?\n/)) {
    if (!line.trim()) continue

    const cells = splitCells(line)
    const name = cells[0]
    if (!name) continue

    // 머리글 줄은 건너뛴다
    if (/^(이름|성명|교사명|번호|no)$/i.test(name)) continue

    const row: ParsedRow = { name, raw: line, subjectNames: [] }
    const leftovers: string[] = []

    for (const cell of cells.slice(1)) {
      if (!cell) continue

      const cls = parseClass(cell)
      if (cls && row.grade == null) {
        row.grade = cls.grade
        row.classNo = cls.classNo
        row.className = cls.className
        continue
      }

      const role = parseRole(cell)
      if (role && row.roleCode == null) {
        row.roleCode = role
        // '보건교사'처럼 구분과 업무가 같은 칸에 있으면 메모로도 남긴다
        if (role === 'OTHER' && cell.replace(/\s/g, '') !== '기타') leftovers.push(cell)
        continue
      }

      leftovers.push(cell)
    }

    // 남은 칸 처리: 쉼표로 나뉜 짧은 낱말은 과목, 그 밖은 메모
    for (const cell of leftovers) {
      const parts = cell
        .split(/[,·/]/)
        .map((p) => p.trim())
        .filter(Boolean)
      const looksLikeSubjects =
        parts.length > 0 && parts.every((p) => p.length <= 8 && !/\d/.test(p))

      if (looksLikeSubjects && (row.roleCode == null || row.roleCode === 'SPECIAL')) {
        row.subjectNames = [...(row.subjectNames ?? []), ...parts]
      } else if (!row.memo) {
        row.memo = cell
      }
    }

    // 구분을 못 찾았으면 내용으로 짐작한다
    if (row.roleCode == null) {
      if (row.grade != null) row.roleCode = 'HOMEROOM'
      else if ((row.subjectNames?.length ?? 0) > 0) row.roleCode = 'SPECIAL'
      else row.roleCode = 'OTHER'
      row.note = '구분을 자동으로 정했습니다'
    }

    // 담임인데 과목만 있으면 과목은 메모로 옮긴다
    if (row.roleCode === 'HOMEROOM' && row.grade == null && (row.subjectNames?.length ?? 0) > 0) {
      row.memo = row.memo ?? row.subjectNames!.join(', ')
      row.subjectNames = []
    }

    out.push(row)
  }

  return out
}
