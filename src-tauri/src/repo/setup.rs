//! 초기 설정 진행 상태 + 단계별 임시 저장(자동 저장).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// 단계 진행 상태.
/// PENDING(아직) / IN_PROGRESS(작성 중) / DONE(완료) / SKIPPED(건너뜀)
pub const STATUS_PENDING: &str = "PENDING";
pub const STATUS_IN_PROGRESS: &str = "IN_PROGRESS";
pub const STATUS_DONE: &str = "DONE";
pub const STATUS_SKIPPED: &str = "SKIPPED";

/// 자동 저장 항목은 settings 테이블에 이 접두사로 보관한다.
const DRAFT_PREFIX: &str = "setup_draft:";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStep {
    pub key: String,
    pub order: i32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupState {
    pub steps: Vec<SetupStep>,
    /// 이어서 진행할 단계. 모두 끝났으면 마지막 단계(DONE).
    pub resume_step: String,
    /// 한 번이라도 입력한 흔적이 있는가 (시작 화면에서 '이어서 진행' 안내 여부)
    pub has_progress: bool,
    /// 초기 설정을 끝냈는가
    pub completed: bool,
    /// 마지막으로 작업한 단계의 이름 (없으면 None)
    pub last_worked_step: Option<String>,
}

pub fn get_state(conn: &Connection) -> AppResult<SetupState> {
    let mut stmt = conn.prepare(
        "SELECT step_key, step_order, status FROM setup_steps ORDER BY step_order",
    )?;
    let steps: Vec<SetupStep> = stmt
        .query_map([], |r| {
            Ok(SetupStep {
                key: r.get(0)?,
                order: r.get(1)?,
                status: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    if steps.is_empty() {
        return Err(AppError::internal("setup_steps 기본 데이터가 없습니다."));
    }

    let completed = is_completed(conn)?;

    // 아직 끝나지 않은 첫 단계
    let resume_step = steps
        .iter()
        .find(|s| s.status != STATUS_DONE && s.status != STATUS_SKIPPED)
        .map(|s| s.key.clone())
        .unwrap_or_else(|| steps.last().expect("not empty").key.clone());

    let has_progress = steps.iter().any(|s| s.status != STATUS_PENDING);

    // 마지막으로 손댄 단계 = updated_at이 가장 최근이면서 PENDING이 아닌 것
    let last_worked_step: Option<String> = conn
        .query_row(
            "SELECT step_key FROM setup_steps
              WHERE status <> ?1
              ORDER BY updated_at DESC, step_order DESC
              LIMIT 1",
            params![STATUS_PENDING],
            |r| r.get(0),
        )
        .optional()?;

    Ok(SetupState {
        steps,
        resume_step,
        has_progress,
        completed,
        last_worked_step,
    })
}

pub fn is_completed(conn: &Connection) -> AppResult<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM app_meta WHERE key = 'setup_completed_at'",
        [],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub fn set_status(conn: &Connection, step_key: &str, status: &str) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE setup_steps
            SET status = ?2, updated_at = datetime('now','localtime')
          WHERE step_key = ?1",
        params![step_key, status],
    )?;
    if n == 0 {
        return Err(AppError::not_found("설정 단계를 찾을 수 없습니다."));
    }
    Ok(())
}

/// 이미 DONE인 단계를 IN_PROGRESS로 되돌리지는 않는다(재편집 중 진행률이 뒷걸음질치지 않도록).
pub fn touch_in_progress(conn: &Connection, step_key: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE setup_steps
            SET status = ?2, updated_at = datetime('now','localtime')
          WHERE step_key = ?1 AND status = ?3",
        params![step_key, STATUS_IN_PROGRESS, STATUS_PENDING],
    )?;
    Ok(())
}

pub fn mark_completed(conn: &Connection) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_meta(key, value, updated_at)
         VALUES ('setup_completed_at', datetime('now','localtime'), datetime('now','localtime'))
         ON CONFLICT(key) DO UPDATE
            SET value = excluded.value, updated_at = excluded.updated_at",
        [],
    )?;
    Ok(())
}

/// 진행 상태만 초기화한다. **입력한 자료는 지우지 않는다.**
/// (사용자가 처음 단계부터 다시 확인하며 넘어갈 수 있도록)
pub fn restart(conn: &Connection) -> AppResult<()> {
    conn.execute(
        "UPDATE setup_steps
            SET status = ?1, updated_at = datetime('now','localtime')",
        params![STATUS_PENDING],
    )?;
    conn.execute("DELETE FROM app_meta WHERE key = 'setup_completed_at'", [])?;
    Ok(())
}

// ---------- 자동 저장(임시 보관) ----------

pub fn save_draft(conn: &Connection, step_key: &str, value_json: &str) -> AppResult<()> {
    // 형식이 깨진 JSON을 넣지 않도록 한 번 검증
    serde_json::from_str::<serde_json::Value>(value_json)?;

    conn.execute(
        "INSERT INTO settings(key, value_json, updated_at)
         VALUES (?1, ?2, datetime('now','localtime'))
         ON CONFLICT(key) DO UPDATE
            SET value_json = excluded.value_json, updated_at = excluded.updated_at",
        params![format!("{DRAFT_PREFIX}{step_key}"), value_json],
    )?;
    Ok(())
}

pub fn get_draft(conn: &Connection, step_key: &str) -> AppResult<Option<String>> {
    let v: Option<String> = conn
        .query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            params![format!("{DRAFT_PREFIX}{step_key}")],
            |r| r.get(0),
        )
        .optional()?;
    Ok(v)
}

/// 설정을 마칠 때 남은 자동저장 초안을 모두 지운다.
///
/// 초안은 '아직 저장하지 않은 입력'을 담아 두는 곳이다. 설정을 마쳤다면
/// 모든 값이 본 자료로 들어갔으므로 초안은 뜻이 없다. 남겨 두면 나중에
/// 설정을 다시 열었을 때 예전 입력이 되살아나 혼란을 준다.
pub fn clear_all_drafts(conn: &Connection) -> AppResult<usize> {
    let n = conn.execute(
        "DELETE FROM settings WHERE key LIKE ?1",
        params![format!("{DRAFT_PREFIX}%")],
    )?;
    Ok(n)
}

pub fn clear_draft(conn: &Connection, step_key: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM settings WHERE key = ?1",
        params![format!("{DRAFT_PREFIX}{step_key}")],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::memory_conn;

    #[test]
    fn 처음에는_모든_단계가_대기중이다() {
        let c = memory_conn();
        let st = get_state(&c).unwrap();

        assert_eq!(st.steps.len(), 6);
        assert!(st.steps.iter().all(|s| s.status == STATUS_PENDING));
        assert_eq!(st.resume_step, "SCHOOL");
        assert!(!st.has_progress);
        assert!(!st.completed);
        assert_eq!(st.last_worked_step, None);
    }

    #[test]
    fn 자동저장하면_진행중으로_바뀌고_이어하기가_가능하다() {
        let c = memory_conn();
        save_draft(&c, "SCHOOL", r#"{"name":"한빛초"}"#).unwrap();
        touch_in_progress(&c, "SCHOOL").unwrap();

        let st = get_state(&c).unwrap();
        assert!(st.has_progress, "입력 흔적이 있으면 '이어서 진행' 안내가 떠야 한다");
        assert_eq!(st.resume_step, "SCHOOL");
        assert_eq!(st.last_worked_step.as_deref(), Some("SCHOOL"));

        let draft = get_draft(&c, "SCHOOL").unwrap().unwrap();
        assert!(draft.contains("한빛초"));
    }

    #[test]
    fn 형식이_깨진_자동저장은_거부한다() {
        let c = memory_conn();
        assert!(save_draft(&c, "SCHOOL", "{not json").is_err());
        assert_eq!(get_draft(&c, "SCHOOL").unwrap(), None);
    }

    #[test]
    fn 완료하거나_건너뛰면_다음_단계로_이어진다() {
        let c = memory_conn();

        set_status(&c, "SCHOOL", STATUS_DONE).unwrap();
        assert_eq!(get_state(&c).unwrap().resume_step, "BELL");

        set_status(&c, "BELL", STATUS_SKIPPED).unwrap();
        assert_eq!(get_state(&c).unwrap().resume_step, "TEACHER");
    }

    #[test]
    fn 이미_완료된_단계는_되돌아가지_않는다() {
        let c = memory_conn();
        set_status(&c, "SCHOOL", STATUS_DONE).unwrap();
        touch_in_progress(&c, "SCHOOL").unwrap();

        let st = get_state(&c).unwrap();
        let school = st.steps.iter().find(|s| s.key == "SCHOOL").unwrap();
        assert_eq!(school.status, STATUS_DONE, "완료된 단계가 진행중으로 뒷걸음질치면 안 된다");
    }

    #[test]
    fn 설정을_마치면_완료로_표시된다() {
        let c = memory_conn();
        assert!(!is_completed(&c).unwrap());

        mark_completed(&c).unwrap();
        assert!(is_completed(&c).unwrap());
        assert!(get_state(&c).unwrap().completed);

        // 두 번 호출해도 오류가 나지 않아야 한다
        mark_completed(&c).unwrap();
        assert!(is_completed(&c).unwrap());
    }

    #[test]
    fn 처음부터_다시_보더라도_입력한_내용은_남는다() {
        let c = memory_conn();
        save_draft(&c, "SCHOOL", r#"{"name":"한빛초"}"#).unwrap();
        set_status(&c, "SCHOOL", STATUS_DONE).unwrap();
        mark_completed(&c).unwrap();

        restart(&c).unwrap();

        let st = get_state(&c).unwrap();
        assert!(st.steps.iter().all(|s| s.status == STATUS_PENDING));
        assert!(!st.completed);
        assert_eq!(st.resume_step, "SCHOOL");

        let draft = get_draft(&c, "SCHOOL").unwrap();
        assert!(draft.is_some(), "진행 상태만 초기화하고 입력 자료는 지우지 않는다");
    }

    /// `setup_complete` 명령이 한 트랜잭션 안에서 하는 일을 그대로 따라간다.
    ///
    /// 실제 화면에서 [설정 마치고 시작하기] 를 눌렀는데 시작 화면이 다시 떠서
    /// 한 번 더 눌러야 했던 문제가 있었다. 그때 남아 있던 초안 때문에
    /// '설정을 이어서 진행할까요?' 가 뜬 것이므로, 마치는 순간
    /// **완료 표시와 초안 삭제가 함께** 되어야 한다.
    #[test]
    fn 설정을_마치면_완료_표시와_초안_삭제가_함께_된다() {
        let c = memory_conn();

        // 여러 단계를 입력하다 만 상태
        save_draft(&c, "SCHOOL", r#"{"name":"한빛초"}"#).unwrap();
        save_draft(&c, "BELL", r#"{"draft":true}"#).unwrap();
        save_draft(&c, "TEACHER", r#"{"draft":true}"#).unwrap();
        for k in ["SCHOOL", "BELL", "TEACHER", "LESSON", "PRIORITY"] {
            set_status(&c, k, STATUS_DONE).unwrap();
        }

        // setup_complete 이 하는 일
        set_status(&c, "DONE", STATUS_DONE).unwrap();
        mark_completed(&c).unwrap();
        let cleared = clear_all_drafts(&c).unwrap();
        assert_eq!(cleared, 3, "남아 있던 초안 세 개가 모두 지워져야 한다");

        let st = get_state(&c).unwrap();
        assert!(st.completed, "한 번에 완료로 바뀌어야 한다");
        assert!(
            st.steps.iter().all(|s| s.status == STATUS_DONE),
            "모든 단계가 완료여야 한다"
        );

        for k in ["SCHOOL", "BELL", "TEACHER"] {
            assert!(
                get_draft(&c, k).unwrap().is_none(),
                "{k} 초안이 남아 있으면 설정 화면이 예전 입력을 되살린다"
            );
        }
    }

    #[test]
    fn 초안이_없어도_지우기는_조용히_끝난다() {
        let c = memory_conn();
        assert_eq!(clear_all_drafts(&c).unwrap(), 0);
    }

    #[test]
    fn 없는_단계는_오류를_낸다() {
        let c = memory_conn();
        assert!(set_status(&c, "NOPE", STATUS_DONE).is_err());
    }
}
