import type { ClassOption, LessonType, SubjectOption } from '@/ipc/lesson'
import { normalizeClassName } from '../../lib/classLabel.ts'

/**
 * 엑셀·한글에서 복사한 전담 시간표 표를 읽는다.
 *
 * 두 가지 배치를 모두 받는다.
 *
 *   (가) 열 = 요일 (가장 흔함)
 *          	월      	화      	수
 *        1교시	3-1 영어	        	4-1 영어
 *        2교시	3-2 영어	4-1 영어
 *
 *   (나) 열 = 교시
 *          	1교시   	2교시   	3교시
 *        월  	3-1 영어	3-2 영어
 *        화  	        	4-1 영어	4-2 영어
 *
 * 칸 안의 표기는 순서에 상관없이 읽는다.
 *   3-1 영어 / 영어 3-1 / 3-1영어 / 3학년 1반 영어 / 3-가람 체육 / 3-1
 */

const DAY_WORDS: Record<string, number> = {
  월: 1, 화: 2, 수: 3, 목: 4, 금: 5, 토: 6, 일: 7,
  월요일: 1, 화요일: 2, 수요일: 3, 목요일: 4, 금요일: 5, 토요일: 6, 일요일: 7,
}

const EMPTY_MARKS = new Set(['', '-', '—', '–', '.', 'x', 'X', '없음', '공강'])

export interface ParsedCell {
  dayOfWeek: number
  periodNo: number
  /** 붙여넣은 원문 */
  raw: string
  /** 찾아낸 학급 */
  classId?: number
  classLabel?: string
  /** 찾아낸 과목 */
  subjectId?: number
  subjectName?: string
  lessonType: LessonType
  /** 못 읽었거나 확인이 필요한 이유 */
  problem?: string
}

export interface ParseResult {
  /** 열이 요일인지 교시인지 */
  layout: 'DAY_COLUMNS' | 'PERIOD_COLUMNS' | 'UNKNOWN'
  cells: ParsedCell[]
  /** 표 자체를 못 읽은 이유 */
  fatal?: string
}

function splitCells(line: string): string[] {
  return (line.includes('\t') ? line.split('\t') : line.split(',')).map((c) => c.trim())
}

function asDay(cell: string): number | null {
  const t = cell.replace(/\s/g, '')
  return DAY_WORDS[t] ?? null
}

function asPeriod(cell: string): number | null {
  const t = cell.replace(/\s/g, '')
  const m = /^(\d{1,2})(교시)?$/.exec(t)
  return m ? Number(m[1]) : null
}

/** 칸 안에서 학급 표기를 찾아낸다. 반환값은 [학년, 반번호|반이름, 남은 글자] */
function findClassToken(
  text: string,
): { grade: number; classNo?: number; className?: string; rest: string } | null {
  const patterns: { re: RegExp; pick: (m: RegExpExecArray) => { grade: number; classNo?: number; className?: string } }[] = [
    // 3학년 1반 / 3학년 가람반
    {
      re: /(\d{1,2})\s*학년\s*(\d{1,2})\s*반/,
      pick: (m) => ({ grade: Number(m[1]), classNo: Number(m[2]) }),
    },
    {
      re: /(\d{1,2})\s*학년\s*([^\d\s]{1,10}?)\s*반/,
      pick: (m) => ({ grade: Number(m[1]), className: m[2] }),
    },
    // 3-1 / 3-1반
    {
      re: /(?<![\d.])(\d{1,2})\s*[-–~]\s*(\d{1,2})\s*반?/,
      pick: (m) => ({ grade: Number(m[1]), classNo: Number(m[2]) }),
    },
    // 3-가람 / 3-가람반
    {
      re: /(?<![\d.])(\d{1,2})\s*[-–~]\s*([^\d\s]{1,10}?)\s*반?(?![^\s])/,
      pick: (m) => ({ grade: Number(m[1]), className: m[2] }),
    },
  ]

  for (const { re, pick } of patterns) {
    const m = re.exec(text)
    if (m) {
      return {
        ...pick(m),
        rest: (text.slice(0, m.index) + ' ' + text.slice(m.index + m[0].length)).trim(),
      }
    }
  }
  return null
}

interface Lookup {
  classes: ClassOption[]
  subjects: SubjectOption[]
  /** 학급만 적혀 있을 때 쓸 기본 과목 */
  defaultSubjectId?: number
  defaultSubjectName?: string
}

function resolveClass(
  found: { grade: number; classNo?: number; className?: string },
  classes: ClassOption[],
): ClassOption | null {
  if (found.className != null) {
    const want = normalizeClassName(found.className)
    return (
      classes.find((c) => c.grade === found.grade && c.name != null && c.name === want) ?? null
    )
  }
  return classes.find((c) => c.grade === found.grade && c.classNo === found.classNo) ?? null
}

function resolveSubject(rest: string, subjects: SubjectOption[]): SubjectOption | null {
  const t = rest.replace(/[()[\]]/g, ' ').trim()
  if (!t) return null
  // 긴 이름을 먼저 맞춰 본다 ('창의적 체험활동'이 '창의'보다 먼저)
  const sorted = [...subjects].sort((a, b) => b.name.length - a.name.length)
  for (const s of sorted) {
    if (t.includes(s.name)) return s
  }
  return null
}

function parseOneCell(
  raw: string,
  dayOfWeek: number,
  periodNo: number,
  look: Lookup,
): ParsedCell | null {
  const text = raw.trim()
  if (EMPTY_MARKS.has(text)) return null

  const cell: ParsedCell = { dayOfWeek, periodNo, raw: text, lessonType: 'SPECIAL' }

  const found = findClassToken(text)
  if (!found) {
    cell.problem = '학급을 못 찾았습니다'
    return cell
  }

  const cls = resolveClass(found, look.classes)
  if (!cls) {
    const asked =
      found.className != null
        ? `${found.grade}학년 ${normalizeClassName(found.className)}반`
        : `${found.grade}학년 ${found.classNo}반`
    cell.problem = `${asked}이 없습니다`
    return cell
  }
  cell.classId = cls.classId
  cell.classLabel = cls.label

  const subject = resolveSubject(found.rest, look.subjects)
  if (subject) {
    cell.subjectId = subject.id
    cell.subjectName = subject.name
  } else if (look.defaultSubjectId != null) {
    cell.subjectId = look.defaultSubjectId
    cell.subjectName = look.defaultSubjectName
  } else if (found.rest) {
    cell.problem = `'${found.rest}'을 과목으로 알아보지 못했습니다`
  }

  return cell
}

export function parseLessonGrid(text: string, look: Lookup): ParseResult {
  const lines = text.split(/\r?\n/).filter((l) => l.trim().length > 0)
  if (lines.length < 2) {
    return { layout: 'UNKNOWN', cells: [], fatal: '표를 읽지 못했습니다. 머리글과 자료 줄이 함께 필요합니다.' }
  }

  const rows = lines.map(splitCells)
  const header = rows[0]

  // 머리글에서 요일이 몇 개인지, 교시가 몇 개인지 세어 배치를 정한다
  const dayCols = header.map(asDay).filter((v) => v != null).length
  const periodCols = header.map(asPeriod).filter((v) => v != null).length

  if (dayCols === 0 && periodCols === 0) {
    return {
      layout: 'UNKNOWN',
      cells: [],
      fatal: '첫 줄에서 요일(월·화·수…)이나 교시(1교시·2교시…)를 찾지 못했습니다. 머리글이 있는 표를 붙여넣어 주세요.',
    }
  }

  const layout = dayCols >= periodCols ? 'DAY_COLUMNS' : 'PERIOD_COLUMNS'
  const cells: ParsedCell[] = []

  for (const row of rows.slice(1)) {
    const first = row[0] ?? ''
    const rowKey = layout === 'DAY_COLUMNS' ? asPeriod(first) : asDay(first)
    if (rowKey == null) continue // 소계 줄 등은 건너뛴다

    for (let col = 1; col < row.length; col++) {
      const colKey =
        layout === 'DAY_COLUMNS' ? asDay(header[col] ?? '') : asPeriod(header[col] ?? '')
      if (colKey == null) continue

      const day = layout === 'DAY_COLUMNS' ? colKey : rowKey
      const period = layout === 'DAY_COLUMNS' ? rowKey : colKey

      const parsed = parseOneCell(row[col] ?? '', day, period, look)
      if (parsed) cells.push(parsed)
    }
  }

  if (cells.length === 0) {
    return {
      layout,
      cells,
      fatal: '표에서 수업을 하나도 찾지 못했습니다. 칸에 3-1 영어처럼 학급과 과목을 적어 주세요.',
    }
  }

  cells.sort((a, b) => a.dayOfWeek - b.dayOfWeek || a.periodNo - b.periodNo)
  return { layout, cells }
}
