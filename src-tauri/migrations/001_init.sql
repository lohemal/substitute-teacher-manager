-- ============================================================
--  보결 배정 시스템 — 초기 스키마 (schema version 1)
--
--  설계 원칙
--   1) 모든 시각은 '자정 기준 분(0~1439)' 정수로 저장한다. (09:00 -> 540)
--   2) 시정표는 프로필 + 학년 매핑 구조. 학년별 요일별 실제 시각을 관리한다.
--   3) 담임 시간표는 저장하지 않고 계산한다.
--      담임 수업 = (자기 반 그 요일 전체 교시) - (그 반에 replaces_homeroom 수업이 있는 교시)
--   4) 보결 기록은 그 시점의 학년/반/교시/시각을 스냅샷으로 복사 저장한다.
--   5) 배정 기준(룰)의 정의는 코드에, 사용 여부와 순서만 DB에 둔다.
-- ============================================================

-- ------------------------------------------------------------
-- 1. 메타 / 설정
-- ------------------------------------------------------------

CREATE TABLE app_meta (
  key        TEXT PRIMARY KEY,
  value      TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);

CREATE TABLE settings (
  key        TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);

CREATE TABLE setup_steps (
  step_key   TEXT PRIMARY KEY,   -- SCHOOL | BELL | TEACHER | LESSON | PRIORITY | DONE
  step_order INTEGER NOT NULL,
  status     TEXT NOT NULL DEFAULT 'PENDING',   -- PENDING | IN_PROGRESS | DONE
  updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);

-- ------------------------------------------------------------
-- 2. 학교 / 학년도·학기 / 학급
-- ------------------------------------------------------------

CREATE TABLE school (
  id          INTEGER PRIMARY KEY CHECK (id = 1),
  name        TEXT    NOT NULL,
  school_type TEXT    NOT NULL DEFAULT 'ELEMENTARY',  -- ELEMENTARY | MIDDLE | HIGH
  min_grade   INTEGER NOT NULL DEFAULT 1,
  max_grade   INTEGER NOT NULL DEFAULT 6,
  school_days TEXT    NOT NULL DEFAULT '1,2,3,4,5',   -- 1=월 … 7=일
  created_at  TEXT    NOT NULL DEFAULT (datetime('now','localtime')),
  updated_at  TEXT    NOT NULL DEFAULT (datetime('now','localtime'))
);

-- 학년도/학기: 시정표·시간표·학급 편성을 묶는 단위.
-- 초기 버전 UI에서는 '현재 학기' 하나만 다룬다.
CREATE TABLE terms (
  id          INTEGER PRIMARY KEY,
  school_year INTEGER NOT NULL,                 -- 2026
  semester    INTEGER NOT NULL,                 -- 1 | 2
  name        TEXT    NOT NULL,                 -- '2026학년도 2학기'
  start_date  TEXT,
  end_date    TEXT,
  is_current  INTEGER NOT NULL DEFAULT 0,       -- 현재 학기 1개만 1
  created_at  TEXT    NOT NULL DEFAULT (datetime('now','localtime')),
  UNIQUE (school_year, semester)
);
CREATE UNIQUE INDEX ux_terms_current ON terms(is_current) WHERE is_current = 1;

CREATE TABLE classes (
  id       INTEGER PRIMARY KEY,
  term_id  INTEGER NOT NULL REFERENCES terms(id) ON DELETE CASCADE,
  grade    INTEGER NOT NULL,
  class_no INTEGER NOT NULL,
  homeroom_teacher_id INTEGER REFERENCES teachers(id) ON DELETE SET NULL,
  active   INTEGER NOT NULL DEFAULT 1,
  UNIQUE (term_id, grade, class_no)
);
CREATE INDEX ix_classes_term     ON classes(term_id, active);
CREATE INDEX ix_classes_homeroom ON classes(homeroom_teacher_id);

-- ------------------------------------------------------------
-- 3. 교사 / 과목
-- ------------------------------------------------------------

CREATE TABLE teacher_roles (
  code       TEXT PRIMARY KEY,       -- HOMEROOM | SPECIAL | OTHER
  label      TEXT NOT NULL,
  sort_order INTEGER NOT NULL,
  is_builtin INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE teachers (
  id               INTEGER PRIMARY KEY,
  name             TEXT    NOT NULL,
  role_code        TEXT    NOT NULL REFERENCES teacher_roles(code),
  is_substitutable INTEGER NOT NULL DEFAULT 1,  -- 보결 배정 대상 여부
  memo             TEXT,                        -- '교감', '보건', '사서' 등
  sort_order       INTEGER NOT NULL DEFAULT 0,
  active           INTEGER NOT NULL DEFAULT 1,  -- 전출/퇴직 시 0 (기록 보존)
  created_at       TEXT    NOT NULL DEFAULT (datetime('now','localtime')),
  updated_at       TEXT    NOT NULL DEFAULT (datetime('now','localtime'))
);
CREATE INDEX ix_teachers_active ON teachers(active, sort_order);

CREATE TABLE subjects (
  id     INTEGER PRIMARY KEY,
  name   TEXT NOT NULL UNIQUE,
  active INTEGER NOT NULL DEFAULT 1
);

-- 한 교사가 여러 과목을 담당할 수 있다 (N:M)
CREATE TABLE teacher_subjects (
  teacher_id INTEGER NOT NULL REFERENCES teachers(id)  ON DELETE CASCADE,
  subject_id INTEGER NOT NULL REFERENCES subjects(id)  ON DELETE CASCADE,
  is_primary INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (teacher_id, subject_id)
);

-- ------------------------------------------------------------
-- 4. 시정표 (프로필 + 학년 매핑)
-- ------------------------------------------------------------

CREATE TABLE bell_schedules (
  id      INTEGER PRIMARY KEY,
  term_id INTEGER NOT NULL REFERENCES terms(id) ON DELETE CASCADE,
  name    TEXT    NOT NULL,          -- '저학년 시정표'
  note    TEXT
);
CREATE INDEX ix_bell_sched_term ON bell_schedules(term_id);

CREATE TABLE bell_slots (
  id               INTEGER PRIMARY KEY,
  bell_schedule_id INTEGER NOT NULL REFERENCES bell_schedules(id) ON DELETE CASCADE,
  day_of_week      INTEGER NOT NULL CHECK (day_of_week BETWEEN 1 AND 7),
  slot_type        TEXT    NOT NULL CHECK (slot_type IN ('PERIOD','LUNCH','OTHER')),
  period_no        INTEGER,          -- slot_type='PERIOD'일 때만
  label            TEXT    NOT NULL, -- '1교시' | '점심'
  start_min        INTEGER NOT NULL CHECK (start_min BETWEEN 0 AND 1439),
  end_min          INTEGER NOT NULL CHECK (end_min   BETWEEN 1 AND 1440),
  CHECK (end_min > start_min),
  CHECK ( (slot_type =  'PERIOD' AND period_no IS NOT NULL)
       OR (slot_type <> 'PERIOD' AND period_no IS NULL) )
);
CREATE UNIQUE INDEX ux_bell_period
  ON bell_slots(bell_schedule_id, day_of_week, period_no) WHERE period_no IS NOT NULL;
CREATE UNIQUE INDEX ux_bell_lunch
  ON bell_slots(bell_schedule_id, day_of_week) WHERE slot_type = 'LUNCH';
CREATE INDEX ix_bell_slots_day ON bell_slots(bell_schedule_id, day_of_week);

-- 학년 -> 시정표 프로필
CREATE TABLE grade_bell_map (
  term_id          INTEGER NOT NULL REFERENCES terms(id) ON DELETE CASCADE,
  grade            INTEGER NOT NULL,
  bell_schedule_id INTEGER NOT NULL REFERENCES bell_schedules(id) ON DELETE CASCADE,
  PRIMARY KEY (term_id, grade)
);

-- ------------------------------------------------------------
-- 5. 수업 시간표 / 교사 개별 일정
-- ------------------------------------------------------------

-- 전담 수업 + 담임 교차수업 + 공동수업.
-- 담임 시간표는 저장하지 않고 이 표의 '빈 교시'로부터 계산한다.
CREATE TABLE lessons (
  id          INTEGER PRIMARY KEY,
  term_id     INTEGER NOT NULL REFERENCES terms(id)    ON DELETE CASCADE,
  teacher_id  INTEGER NOT NULL REFERENCES teachers(id) ON DELETE CASCADE,
  day_of_week INTEGER NOT NULL CHECK (day_of_week BETWEEN 1 AND 7),
  period_no   INTEGER NOT NULL,
  class_id    INTEGER NOT NULL REFERENCES classes(id)  ON DELETE CASCADE,
  subject_id  INTEGER REFERENCES subjects(id) ON DELETE SET NULL,
  -- SPECIAL 전담수업 / CROSS 담임 교차수업 / CO 공동수업 / ETC 기타
  lesson_type TEXT    NOT NULL DEFAULT 'SPECIAL'
              CHECK (lesson_type IN ('SPECIAL','CROSS','CO','ETC')),
  -- 1이면 이 교시에 그 반 담임은 수업하지 않는다(담임 시간표 계산에서 제외).
  -- 공동수업(CO)은 담임도 함께 들어가므로 0.
  replaces_homeroom INTEGER NOT NULL DEFAULT 1,
  note        TEXT,
  -- 한 교사가 같은 시간에 두 수업을 할 수는 없다
  UNIQUE (term_id, teacher_id, day_of_week, period_no)
);
CREATE INDEX ix_lessons_class   ON lessons(term_id, class_id, day_of_week, period_no);
CREATE INDEX ix_lessons_teacher ON lessons(term_id, teacher_id, day_of_week);

-- 수업 외 일정: 고정 업무 / 특정 시간 보결 배정 불가 / 기타 정기 일정
CREATE TABLE teacher_busy_blocks (
  id            INTEGER PRIMARY KEY,
  term_id       INTEGER REFERENCES terms(id) ON DELETE CASCADE,
  teacher_id    INTEGER NOT NULL REFERENCES teachers(id) ON DELETE CASCADE,
  -- DUTY 고정 업무 / NO_SUB 보결 배정 불가 / OTHER 기타
  block_type    TEXT    NOT NULL DEFAULT 'DUTY'
                CHECK (block_type IN ('DUTY','NO_SUB','OTHER')),
  recurrence    TEXT    NOT NULL CHECK (recurrence IN ('WEEKLY','ONCE')),
  day_of_week   INTEGER,          -- recurrence='WEEKLY'
  specific_date TEXT,             -- recurrence='ONCE'  (YYYY-MM-DD)
  start_min     INTEGER NOT NULL CHECK (start_min BETWEEN 0 AND 1439),
  end_min       INTEGER NOT NULL CHECK (end_min   BETWEEN 1 AND 1440),
  label         TEXT    NOT NULL, -- '업무 협의회', '순회 지도'
  active        INTEGER NOT NULL DEFAULT 1,
  created_at    TEXT    NOT NULL DEFAULT (datetime('now','localtime')),
  CHECK (end_min > start_min),
  CHECK ( (recurrence = 'WEEKLY' AND day_of_week   IS NOT NULL)
       OR (recurrence = 'ONCE'   AND specific_date IS NOT NULL) )
);
CREATE INDEX ix_busy_teacher ON teacher_busy_blocks(teacher_id, active);
CREATE INDEX ix_busy_weekly  ON teacher_busy_blocks(day_of_week, active)   WHERE recurrence = 'WEEKLY';
CREATE INDEX ix_busy_once    ON teacher_busy_blocks(specific_date, active) WHERE recurrence = 'ONCE';

-- ------------------------------------------------------------
-- 6. 부재(결근) / 보결
-- ------------------------------------------------------------

CREATE TABLE absence_reasons (
  code       TEXT PRIMARY KEY,
  label      TEXT NOT NULL,
  sort_order INTEGER NOT NULL,
  is_builtin INTEGER NOT NULL DEFAULT 1,
  active     INTEGER NOT NULL DEFAULT 1
);

-- 교사 부재. 종일 부재와 일부 시간 부재를 모두 표현한다.
CREATE TABLE absences (
  id          INTEGER PRIMARY KEY,
  term_id     INTEGER REFERENCES terms(id) ON DELETE SET NULL,
  teacher_id  INTEGER NOT NULL REFERENCES teachers(id),
  date        TEXT    NOT NULL,                  -- YYYY-MM-DD
  is_all_day  INTEGER NOT NULL DEFAULT 1,
  start_min   INTEGER,                           -- is_all_day=0 일 때 필수
  end_min     INTEGER,
  reason_code TEXT REFERENCES absence_reasons(code),
  reason_text TEXT,
  status      TEXT    NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','CANCELLED')),
  created_at  TEXT    NOT NULL DEFAULT (datetime('now','localtime')),
  cancelled_at TEXT,
  CHECK ( is_all_day = 1
       OR (start_min IS NOT NULL AND end_min IS NOT NULL AND end_min > start_min) )
);
CREATE INDEX ix_absence_date    ON absences(date, status);
CREATE INDEX ix_absence_teacher ON absences(teacher_id, date, status);

-- 보결 배정 기록. 학년/반/교시/시각은 그 시점 값을 스냅샷으로 복사한다.
CREATE TABLE substitutions (
  id          INTEGER PRIMARY KEY,
  term_id     INTEGER REFERENCES terms(id) ON DELETE SET NULL,
  absence_id  INTEGER REFERENCES absences(id) ON DELETE SET NULL,  -- 일괄 보결 묶음
  date        TEXT    NOT NULL,                  -- YYYY-MM-DD
  day_of_week INTEGER NOT NULL,

  -- 대상 (스냅샷)
  class_id    INTEGER REFERENCES classes(id) ON DELETE SET NULL,
  grade       INTEGER NOT NULL,
  class_no    INTEGER NOT NULL,

  -- 시간 (스냅샷)
  slot_type   TEXT    NOT NULL CHECK (slot_type IN ('PERIOD','LUNCH')),
  period_no   INTEGER,
  slot_label  TEXT    NOT NULL,                  -- '6교시' | '점심'
  start_min   INTEGER NOT NULL,
  end_min     INTEGER NOT NULL,

  -- 교사
  absent_teacher_id INTEGER REFERENCES teachers(id),
  sub_teacher_id    INTEGER NOT NULL REFERENCES teachers(id),
  subject_id        INTEGER REFERENCES subjects(id) ON DELETE SET NULL,

  -- 사유 / 상태
  reason_code TEXT REFERENCES absence_reasons(code),
  reason_text TEXT,
  status      TEXT    NOT NULL DEFAULT 'ASSIGNED' CHECK (status IN ('ASSIGNED','CANCELLED')),

  -- 감사 정보 (추천 품질 점검용)
  recommend_rank   INTEGER,
  recommend_reason TEXT,

  created_at   TEXT NOT NULL DEFAULT (datetime('now','localtime')),
  cancelled_at TEXT,
  cancel_reason TEXT,

  CHECK (end_min > start_min)
);
CREATE INDEX ix_sub_date    ON substitutions(date, status);
CREATE INDEX ix_sub_subt    ON substitutions(sub_teacher_id, date, status);
CREATE INDEX ix_sub_absent  ON substitutions(absent_teacher_id, date, status);
CREATE INDEX ix_sub_month   ON substitutions(substr(date,1,7), status);
CREATE INDEX ix_sub_absence ON substitutions(absence_id);

-- 같은 교사가 같은 날 같은 시작시각에 두 번 배정되는 것을 DB 차원에서도 막는다.
-- (완전한 구간 중복 검사는 애플리케이션에서 수행)
CREATE UNIQUE INDEX ux_sub_no_double_book
  ON substitutions(sub_teacher_id, date, start_min) WHERE status = 'ASSIGNED';

-- 같은 학급의 같은 시간에 보결이 두 건 생기는 것도 막는다.
CREATE UNIQUE INDEX ux_sub_no_double_class
  ON substitutions(date, grade, class_no, start_min) WHERE status = 'ASSIGNED';

-- ------------------------------------------------------------
-- 7. 배정 기준 (룰의 정의는 코드, 사용 여부/순서만 DB)
-- ------------------------------------------------------------

CREATE TABLE exclusion_rules (
  rule_key    TEXT PRIMARY KEY,
  enabled     INTEGER NOT NULL DEFAULT 1,
  params_json TEXT    NOT NULL DEFAULT '{}',
  updated_at  TEXT    NOT NULL DEFAULT (datetime('now','localtime'))
);

CREATE TABLE priority_rules (
  rule_key    TEXT PRIMARY KEY,
  enabled     INTEGER NOT NULL DEFAULT 0,
  sort_order  INTEGER NOT NULL,
  params_json TEXT    NOT NULL DEFAULT '{}',
  updated_at  TEXT    NOT NULL DEFAULT (datetime('now','localtime'))
);

-- ============================================================
--  기본 데이터
-- ============================================================

INSERT INTO app_meta(key, value) VALUES
  ('schema_version', '1'),
  ('created_at', datetime('now','localtime'));

INSERT INTO settings(key, value_json) VALUES
  ('derive_homeroom_schedule',       'true'),   -- 담임 시간표 자동 계산
  ('exclude_homeroom_on_own_lunch',  'true'),   -- 자기 학년 점심시간에 담임 제외
  ('exclude_absent_teacher_all_day', 'true'),   -- 결근 교사는 그날 전체 제외
  ('show_excluded_teachers',         'true'),   -- 제외된 교사도 사유와 함께 표시
  ('count_total_scope',              '"ALL"'),  -- 누적 집계 범위: ALL | TERM
  ('update_check_interval_hours',    '24');

INSERT INTO setup_steps(step_key, step_order, status) VALUES
  ('SCHOOL',   1, 'PENDING'),
  ('BELL',     2, 'PENDING'),
  ('TEACHER',  3, 'PENDING'),
  ('LESSON',   4, 'PENDING'),
  ('PRIORITY', 5, 'PENDING'),
  ('DONE',     6, 'PENDING');

INSERT INTO teacher_roles(code, label, sort_order) VALUES
  ('HOMEROOM', '담임', 1),
  ('SPECIAL',  '전담', 2),
  ('OTHER',    '기타', 3);

INSERT INTO subjects(name) VALUES
  ('국어'),('수학'),('사회'),('과학'),('영어'),
  ('체육'),('음악'),('미술'),('실과'),('도덕'),
  ('창의적 체험활동');

INSERT INTO absence_reasons(code, label, sort_order) VALUES
  ('ANNUAL',   '연가',   1),
  ('SICK',     '병가',   2),
  ('OFFICIAL', '공가',   3),
  ('TRIP',     '출장',   4),
  ('TRAINING', '연수',   5),
  ('SPECIAL',  '특별휴가', 6),
  ('ETC',      '기타',   9);

-- 제외 조건: 앞의 3개는 항상 켜짐(코드에서 강제), 나머지는 학교가 선택
INSERT INTO exclusion_rules(rule_key, enabled, params_json) VALUES
  ('NOT_SUBSTITUTABLE', 1, '{}'),
  ('IS_ABSENT',         1, '{}'),
  ('TIME_CONFLICT',     1, '{}'),
  ('SAME_GRADE_ONLY',   0, '{}'),
  ('MAX_PER_DAY',       0, '{"max":2}'),
  ('MAX_CONSECUTIVE',   0, '{"max":2}');

-- 추천 우선순위 기본 프리셋: 동학년 -> 당일 적은 순 -> 누적 적은 순
INSERT INTO priority_rules(rule_key, enabled, sort_order, params_json) VALUES
  ('SAME_GRADE',         1, 1, '{}'),
  ('FEWEST_TODAY',       1, 2, '{}'),
  ('FEWEST_TOTAL',       1, 3, '{}'),
  ('FEWEST_MONTH',       0, 4, '{}'),
  ('PREFER_SPECIAL',     0, 5, '{}'),
  ('PREFER_FINISHED',    0, 6, '{}'),
  ('PREFER_LOWER_GRADE', 0, 7, '{}'),
  ('SAME_SUBJECT',       0, 8, '{}'),
  ('PREFER_ADJACENT',    0, 9, '{}');
