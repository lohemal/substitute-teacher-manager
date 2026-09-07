/**
 * 숫자 입력칸이 몇 글자까지 받아야 하는가.
 *
 * `NumberInput` 은 예전에 `maxLength={4}` 로 못 박혀 있었다. 교시 수나 분처럼
 * 작은 값만 넣던 동안에는 문제가 없었지만, **1회 보결 수당(15,000원)** 처럼
 * 다섯 자리 이상인 값이 생기자 넣을 수 없었다.
 *
 * 그래서 자릿수는 그 칸의 `max` 가 정한다. 칸마다 따로 적지 않으므로 새 칸을
 * 만들 때 이 규칙을 잊을 일이 없다.
 */
export function maxDigits(min: number, max: number): number {
  const len = (v: number) => String(Math.trunc(Math.abs(v))).length
  // 음수를 넣을 수 있는 칸은 부호 한 자리를 더 받는다
  return Math.max(len(min), len(max)) + (min < 0 ? 1 : 0)
}
