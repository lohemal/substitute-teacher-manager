-- ============================================================
--  전담 시간표: 겹침 판단을 교시 번호에서 실제 시각으로 (schema version 3)
--
--  001에서는 `UNIQUE (term_id, teacher_id, day_of_week, period_no)` 로
--  '한 교사가 같은 교시에 두 수업'을 막았다. 그런데 학년마다 교시 시각이
--  다르므로 이 제약은 두 가지로 어긋난다.
--
--   (1) 너무 엄격 — 저학년 5교시(13:00~13:40)와 고학년 5교시(12:15~12:55)는
--       시간이 겹치지 않으므로 한 교사가 둘 다 맡을 수 있는데 막혔다.
--   (2) 너무 느슨 — 교시 번호가 달라도 실제 시각이 겹칠 수 있는데 못 막았다.
--
--  그래서 DB 제약은 '똑같은 수업의 중복 입력'만 막고, 실제 시간 겹침은
--  `repo::lesson`에서 학년별 시정표의 시작·종료 시각으로 판단한다.
--  (SQLite로는 구간 겹침을 제약으로 표현할 수 없다)
--
--  같은 학급·같은 교시에 교사가 둘 이상인 경우(합반·공동수업)는 그대로 허용한다.
-- ============================================================

CREATE TABLE lessons_new (
  id INTEGER PRIMARY KEY,
  term_id     INTEGER NOT NULL REFERENCES terms(id)    ON DELETE CASCADE,
  teacher_id  INTEGER NOT NULL REFERENCES teachers(id) ON DELETE CASCADE,
  day_of_week INTEGER NOT NULL CHECK (day_of_week BETWEEN 1 AND 7),
  period_no   INTEGER NOT NULL,
  class_id    INTEGER NOT NULL REFERENCES classes(id)  ON DELETE CASCADE,
  subject_id  INTEGER REFERENCES subjects(id) ON DELETE SET NULL,
  lesson_type TEXT    NOT NULL DEFAULT 'SPECIAL'
              CHECK (lesson_type IN ('SPECIAL','CROSS','CO','ETC')),
  replaces_homeroom INTEGER NOT NULL DEFAULT 1,
  note        TEXT,
  -- 똑같은 수업을 두 번 넣는 것만 막는다
  UNIQUE (term_id, teacher_id, day_of_week, period_no, class_id)
);

INSERT INTO lessons_new
  (id, term_id, teacher_id, day_of_week, period_no, class_id,
   subject_id, lesson_type, replaces_homeroom, note)
SELECT
   id, term_id, teacher_id, day_of_week, period_no, class_id,
   subject_id, lesson_type, replaces_homeroom, note
  FROM lessons;

DROP TABLE lessons;
ALTER TABLE lessons_new RENAME TO lessons;

CREATE INDEX ix_lessons_class   ON lessons(term_id, class_id, day_of_week, period_no);
CREATE INDEX ix_lessons_teacher ON lessons(term_id, teacher_id, day_of_week);
