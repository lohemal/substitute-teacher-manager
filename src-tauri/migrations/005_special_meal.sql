-- 전담교사 식사시간.
--
-- ## 무엇을 저장하고 무엇을 저장하지 않는가
--
-- **자동 판정 결과는 저장하지 않는다.** 식사시간 후보는 학년별 시정표의
-- 점심 구간에서 나오므로, 시정표를 고치면 판정도 따라 바뀌어야 한다. 계산해
-- 둔 값을 들고 있으면 시정표가 바뀐 뒤에도 옛 값으로 판정하게 된다.
--
-- 그래서 남기는 것은 사람이 직접 정한 두 가지뿐이다.
--   1. 학교 기본 식사시간  → settings 의 'meal_default_bell_id'
--      (시각이 아니라 '어느 시정표의 점심시간에 함께 먹는가' 를 가리킨다.
--       요일마다 점심시간이 다른 학교도 그대로 따라간다.)
--   2. 교사별·요일별 수동 지정 → 이 표
--
-- ## 왜 표인가
--
-- 교사 × 요일이라 관계형이 자연스럽고, 교사를 지우면 함께 사라져야 한다.
-- 설정 JSON 에 밀어 넣으면 교사 삭제와 어긋난 값이 남는다.

-- IF NOT EXISTS 인 이유: 실제 자료 보존 시험이 복사본에서 마이그레이션을
-- 다시 돌려 본다. 그때 두 번 실행되어도 탈이 없어야 한다.
CREATE TABLE IF NOT EXISTS teacher_meal_overrides (
  term_id     INTEGER NOT NULL REFERENCES terms(id)    ON DELETE CASCADE,
  teacher_id  INTEGER NOT NULL REFERENCES teachers(id) ON DELETE CASCADE,
  day_of_week INTEGER NOT NULL CHECK (day_of_week BETWEEN 1 AND 7),
  start_min   INTEGER NOT NULL CHECK (start_min BETWEEN 0 AND 1439),
  end_min     INTEGER NOT NULL CHECK (end_min   BETWEEN 1 AND 1440),
  updated_at  TEXT    NOT NULL DEFAULT (datetime('now','localtime')),
  PRIMARY KEY (term_id, teacher_id, day_of_week),
  CHECK (end_min > start_min)
);

CREATE INDEX IF NOT EXISTS ix_meal_override_day
  ON teacher_meal_overrides(term_id, day_of_week);
