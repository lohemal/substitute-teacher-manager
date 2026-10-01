/**
 * 개발용 진단 화면을 보여 줄 것인가.
 *
 * ## 왜 필요한가
 *
 * 후보 판정이 이상할 때 `EXCLUDED_REGULAR_CLASS` 같은 내부 코드와 그 날
 * 교사별 일정을 펼쳐 보면 원인을 바로 찾을 수 있다. 만드는 동안 아주
 * 쓸모가 있었다. 하지만 학교에서 쓰는 분들께는 뜻이 없는 영어 코드일
 * 뿐이라 배포본에서는 보이지 않아야 한다.
 *
 * ## 기준
 *
 * Vite 가 넣어 주는 `import.meta.env.DEV` 하나만 본다. 따로 환경 변수를
 * 만들거나 설정에 항목을 더하지 않는다 — 켜고 끌 일이 아니라 빌드의
 * 성격이기 때문이다.
 *
 * ```text
 * npm run tauri build    (배포 설치본)   false  -> 숨김
 * npm run tauri dev                      true   -> 보임
 * npm run app:sandbox    (연습용)        true   -> 보임
 * ```
 *
 * 연습용(`app:sandbox`)은 `tauri dev` 에 설정 파일만 바꿔 붙인 것이라
 * 따로 손대지 않아도 개발 빌드로 돈다.
 *
 * ## 기능 자체는 지우지 않는다
 *
 * 판정 코드(`reasonCode`)와 교사별 일정(`blocks`)은 서버에서 그대로
 * 내려온다. 화면에 그릴지만 여기서 가른다. 그래야 문제가 생겼을 때
 * 개발 빌드로 같은 자료를 열어 바로 볼 수 있다.
 */
export const DEV_TOOLS: boolean = import.meta.env.DEV
