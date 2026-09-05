/**
 * 학급 표시 이름. Rust `src-tauri/src/label.rs`와 같은 규칙을 쓴다.
 *
 *   이름 있음 : 5-가람   / 5학년 가람반
 *   이름 없음 : 5-1      / 5학년 1반
 */

/** 사용자가 '가람반'이라고 입력해도 '가람'으로 보관한다. */
export function normalizeClassName(raw: string): string {
  const t = raw.trim()
  const stripped = t.endsWith('반') ? t.slice(0, -1).trim() : t
  return stripped.length > 0 ? stripped : t
}

/** 표·칩처럼 좁은 곳에서 쓰는 짧은 표기 */
export function classShort(grade: number, classNo: number, name?: string | null): string {
  const n = name?.trim()
  return n ? `${grade}-${n}` : `${grade}-${classNo}`
}

/** 안내 문장에서 쓰는 긴 표기 */
export function classFull(grade: number, classNo: number, name?: string | null): string {
  const n = name?.trim()
  return n ? `${grade}학년 ${n}반` : `${grade}학년 ${classNo}반`
}

/** 반만 짧게. 목록에서 학년이 이미 보일 때 쓴다. `가람반` / `1반` */
export function classOnly(classNo: number, name?: string | null): string {
  const n = name?.trim()
  return n ? `${n}반` : `${classNo}반`
}
