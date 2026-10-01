-- ------------------------------------------------------------
--  보결 불필요 (v0.1.11)
-- ------------------------------------------------------------
--
-- 결근을 등록하면 그 선생님이 그 날 맡은 시간에서 보결 필요 교시를 계산해
-- 낸다. 그런데 등록한 뒤에 그 날 일정이 바뀌는 일이 있다. 가장 흔한 것이
-- 전담시간 변경이다 — 3교시에 전담 선생님이 들어오기로 바뀌면 그 교시는
-- 더 이상 보결이 필요 없다.
--
-- 지금까지는 그런 교시가 계속 '미배정'으로 남아, 실제로는 처리할 일이
-- 없는데도 현황의 미배정 숫자에 들어갔다.
--
-- ## 왜 새 표인가
--
-- substitutions 를 재활용할 수 없다. 저 표는 **누가 대신 들어갔는지**를
-- 적는 곳이라 sub_teacher_id 가 NOT NULL 이다. 보결 불필요에는 대신
-- 들어가는 사람이 없다. 'CANCELLED' 를 빌려 쓰는 것도 안 된다 —
--   배정 취소  = 사람을 넣었다가 그 배정을 물렸다
--   보결 불필요 = 애초에 아무도 넣을 필요가 없어졌다
-- 두 가지는 뜻이 다르고, 섞으면 나중에 기록을 읽을 수 없다.
--
-- 보결 필요 교시 자체도 저장된 적이 없다. 결근과 시간표에서 그때그때
-- 계산한다. 그래서 '그 가운데 하나가 필요 없어졌다'는 사실만 따로 적는
-- 표가 가장 자연스럽다.
--
-- ## 어떤 칸인지 가리키는 방법
--
-- (date, class_id, start_min). 보결 현황(stats::needs_in)과 다건 배정
-- (assign::day_plan)이 이미 배정 여부를 맞출 때 쓰는 것과 같은 열쇠다.
-- 그래서 한 곳만 보고도 두 화면이 같은 상태를 보여 준다.
--
-- ## 지우지 않는다
--
-- 되돌리기는 행을 지우는 것이 아니라 status 를 REVOKED 로 바꾸는 것이다.
-- substitutions 의 ASSIGNED/CANCELLED 와 같은 방식이라, 언제 누가 무엇을
-- 왜 했는지가 남는다.

CREATE TABLE IF NOT EXISTS substitution_waivers (
  id          INTEGER PRIMARY KEY,
  term_id     INTEGER REFERENCES terms(id) ON DELETE SET NULL,
  absence_id  INTEGER REFERENCES absences(id) ON DELETE SET NULL,
  date        TEXT    NOT NULL,                  -- YYYY-MM-DD
  day_of_week INTEGER NOT NULL,

  -- 대상 (스냅샷 — 반 이름이 바뀌어도 기록은 그대로다)
  class_id    INTEGER REFERENCES classes(id) ON DELETE SET NULL,
  grade       INTEGER NOT NULL,
  class_no    INTEGER NOT NULL,
  class_label TEXT    NOT NULL,

  -- 시간 (스냅샷)
  slot_type   TEXT    NOT NULL CHECK (slot_type IN ('PERIOD','LUNCH')),
  period_no   INTEGER,
  slot_label  TEXT    NOT NULL,
  start_min   INTEGER NOT NULL,
  end_min     INTEGER NOT NULL,

  -- 누구의 보결이었나 (결근 기록은 그대로 둔다)
  absent_teacher_id INTEGER REFERENCES teachers(id),

  -- 왜 필요 없어졌나
  reason_code TEXT    NOT NULL CHECK (reason_code IN ('SPECIAL_CHANGED','OTHER')),
  note        TEXT,

  status      TEXT    NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','REVOKED')),
  created_at  TEXT    NOT NULL DEFAULT (datetime('now','localtime')),
  revoked_at  TEXT,

  CHECK (end_min > start_min)
);

-- 한 칸에 살아 있는 보결 불필요는 하나뿐이다. 되돌린 기록은 남아 있어도
-- 다시 처리할 수 있다.
CREATE UNIQUE INDEX IF NOT EXISTS ux_waiver_slot
  ON substitution_waivers(date, class_id, start_min) WHERE status = 'ACTIVE';

CREATE INDEX IF NOT EXISTS ix_waiver_date ON substitution_waivers(date, status);
CREATE INDEX IF NOT EXISTS ix_waiver_absence ON substitution_waivers(absence_id);
