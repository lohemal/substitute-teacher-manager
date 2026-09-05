//! 추천 기준 설정 저장.
//!
//! **DB에는 어떤 기준을 쓰는지와 순서만 담는다.** 기준의 정의(이름·설명·비교 함수)는
//! `domain::priority`의 코드에 있다. 그래서 새 기준을 추가해도 DB 구조를 바꾸지 않고,
//! 프로그램을 업데이트해도 학교가 정한 순서는 `rule_key` 기준으로 그대로 남는다.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::domain::priority::{all_rules, is_known_key, RuleSetting, DEFAULT_ORDER, TIE_BREAK_TEXT};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleView {
    pub rule_key: String,
    pub label: String,
    pub hint: String,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriorityView {
    pub rules: Vec<RuleView>,
    /// 마지막 동점 처리 기준 (사람 말로)
    pub tie_break: String,
    /// 기본값과 같은 상태인가
    pub is_default: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInput {
    pub rule_key: String,
    pub enabled: bool,
}

/// 정렬 엔진이 쓰는 설정. 프로그램이 아는 기준만 돌려준다.
pub fn settings(conn: &Connection) -> AppResult<Vec<RuleSetting>> {
    let mut stmt = conn
        .prepare("SELECT rule_key, enabled, sort_order FROM priority_rules ORDER BY sort_order")?;
    let rows: Vec<RuleSetting> = stmt
        .query_map([], |r| {
            Ok(RuleSetting {
                rule_key: r.get(0)?,
                enabled: r.get::<_, i64>(1)? != 0,
                sort_order: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // 프로그램이 모르는 키(옛 버전에서 남은 것 등)는 무시한다
    let known: Vec<RuleSetting> = rows
        .into_iter()
        .filter(|s| is_known_key(&s.rule_key))
        .collect();
    Ok(known)
}

pub fn view(conn: &Connection) -> AppResult<PriorityView> {
    let saved = settings(conn)?;
    let registry = all_rules();

    // 저장된 순서를 따르고, 아직 저장되지 않은 기준은 뒤에 붙인다
    let mut rules: Vec<RuleView> = Vec::new();
    for s in &saved {
        if let Some(r) = registry.iter().find(|r| r.key() == s.rule_key) {
            rules.push(RuleView {
                rule_key: r.key().to_string(),
                label: r.label().to_string(),
                hint: r.hint().to_string(),
                enabled: s.enabled,
                sort_order: rules.len() as i32 + 1,
            });
        }
    }
    for r in &registry {
        if !rules.iter().any(|x| x.rule_key == r.key()) {
            rules.push(RuleView {
                rule_key: r.key().to_string(),
                label: r.label().to_string(),
                hint: r.hint().to_string(),
                enabled: false,
                sort_order: rules.len() as i32 + 1,
            });
        }
    }

    let is_default = rules.len() == DEFAULT_ORDER.len()
        && rules
            .iter()
            .zip(DEFAULT_ORDER.iter())
            .all(|(r, (key, on))| r.rule_key == *key && r.enabled == *on);

    Ok(PriorityView {
        rules,
        tie_break: TIE_BREAK_TEXT.to_string(),
        is_default,
    })
}

/// 화면에서 정한 순서와 사용 여부를 그대로 저장한다.
pub fn save(conn: &Connection, rules: &[RuleInput]) -> AppResult<()> {
    if rules.is_empty() {
        return Err(AppError::invalid("추천 기준 목록이 비어 있습니다."));
    }
    for r in rules {
        if !is_known_key(&r.rule_key) {
            return Err(AppError::invalid("알 수 없는 추천 기준입니다."));
        }
    }
    if !rules.iter().any(|r| r.enabled) {
        return Err(AppError::invalid(
            "추천 기준을 하나 이상 켜 주세요. 모두 끄면 추천 순서를 정할 수 없습니다.",
        ));
    }

    for (i, r) in rules.iter().enumerate() {
        conn.execute(
            "INSERT INTO priority_rules(rule_key, enabled, sort_order, updated_at)
             VALUES (?1, ?2, ?3, datetime('now','localtime'))
             ON CONFLICT(rule_key) DO UPDATE SET
                enabled = excluded.enabled,
                sort_order = excluded.sort_order,
                updated_at = excluded.updated_at",
            params![r.rule_key, r.enabled as i64, i as i32 + 1],
        )?;
    }
    Ok(())
}

/// 처음 설치했을 때의 기준으로 되돌린다.
pub fn reset(conn: &Connection) -> AppResult<()> {
    for (i, (key, on)) in DEFAULT_ORDER.iter().enumerate() {
        conn.execute(
            "INSERT INTO priority_rules(rule_key, enabled, sort_order, updated_at)
             VALUES (?1, ?2, ?3, datetime('now','localtime'))
             ON CONFLICT(rule_key) DO UPDATE SET
                enabled = excluded.enabled,
                sort_order = excluded.sort_order,
                updated_at = excluded.updated_at",
            params![key, *on as i64, i as i32 + 1],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::memory_conn;

    fn input(keys: &[(&str, bool)]) -> Vec<RuleInput> {
        keys.iter()
            .map(|(k, on)| RuleInput {
                rule_key: k.to_string(),
                enabled: *on,
            })
            .collect()
    }

    fn order_of(conn: &Connection) -> Vec<String> {
        settings(conn)
            .unwrap()
            .into_iter()
            .filter(|s| s.enabled)
            .map(|s| s.rule_key)
            .collect()
    }

    #[test]
    fn 처음에는_기본_기준이_들어_있다() {
        let c = memory_conn();
        let v = view(&c).unwrap();

        assert!(v.is_default, "설치 직후에는 기본값이어야 한다");
        assert_eq!(order_of(&c), vec!["SAME_GRADE", "FEWEST_TODAY", "FEWEST_TOTAL"]);
        assert!(!v.tie_break.is_empty());
    }

    #[test]
    fn 모든_기준이_이름과_설명을_갖고_목록에_나온다() {
        let c = memory_conn();
        let v = view(&c).unwrap();
        assert_eq!(v.rules.len(), 7, "기본 제공 기준 7가지");
        for r in &v.rules {
            assert!(!r.label.is_empty(), "{}", r.rule_key);
            assert!(!r.hint.is_empty(), "{}", r.rule_key);
        }
        // 순위 번호가 1부터 이어진다
        for (i, r) in v.rules.iter().enumerate() {
            assert_eq!(r.sort_order, i as i32 + 1);
        }
    }

    #[test]
    fn 순서를_바꿔_저장하면_그대로_남는다() {
        let c = memory_conn();
        save(
            &c,
            &input(&[
                ("FEWEST_TOTAL", true),
                ("FEWEST_TODAY", true),
                ("SAME_GRADE", true),
                ("FEWEST_MONTH", false),
                ("PREFER_SPECIAL", false),
                ("PREFER_FINISHED", false),
                ("PREFER_LOWER_GRADE", false),
            ]),
        )
        .unwrap();

        assert_eq!(
            order_of(&c),
            vec!["FEWEST_TOTAL", "FEWEST_TODAY", "SAME_GRADE"],
            "저장한 순서 그대로"
        );

        // 다시 읽어도 같다 (앱을 다시 켠 것과 같은 상황)
        let v = view(&c).unwrap();
        assert_eq!(v.rules[0].rule_key, "FEWEST_TOTAL");
        assert_eq!(v.rules[2].rule_key, "SAME_GRADE");
        assert!(!v.is_default, "기본값과 다르므로 표시가 바뀌어야 한다");
    }

    #[test]
    fn 기준을_끄면_정렬에_쓰이지_않는다() {
        let c = memory_conn();
        save(
            &c,
            &input(&[
                ("SAME_GRADE", false),
                ("FEWEST_TOTAL", true),
                ("FEWEST_TODAY", false),
                ("FEWEST_MONTH", false),
                ("PREFER_SPECIAL", false),
                ("PREFER_FINISHED", false),
                ("PREFER_LOWER_GRADE", false),
            ]),
        )
        .unwrap();

        assert_eq!(order_of(&c), vec!["FEWEST_TOTAL"]);
        // 꺼진 기준도 목록에는 남아 있어야 한다 (다시 켤 수 있게)
        let v = view(&c).unwrap();
        assert_eq!(v.rules.len(), 7);
        assert!(!v.rules.iter().find(|r| r.rule_key == "SAME_GRADE").unwrap().enabled);
    }

    #[test]
    fn 기본값으로_되돌릴_수_있다() {
        let c = memory_conn();
        save(
            &c,
            &input(&[
                ("PREFER_LOWER_GRADE", true),
                ("SAME_GRADE", false),
                ("FEWEST_TODAY", false),
                ("FEWEST_MONTH", false),
                ("FEWEST_TOTAL", false),
                ("PREFER_SPECIAL", false),
                ("PREFER_FINISHED", false),
            ]),
        )
        .unwrap();
        assert!(!view(&c).unwrap().is_default);

        reset(&c).unwrap();

        assert!(view(&c).unwrap().is_default);
        assert_eq!(order_of(&c), vec!["SAME_GRADE", "FEWEST_TODAY", "FEWEST_TOTAL"]);
    }

    #[test]
    fn 모든_기준을_끄면_저장을_막는다() {
        let c = memory_conn();
        let e = save(
            &c,
            &input(&[
                ("SAME_GRADE", false),
                ("FEWEST_TODAY", false),
                ("FEWEST_TOTAL", false),
                ("FEWEST_MONTH", false),
                ("PREFER_SPECIAL", false),
                ("PREFER_FINISHED", false),
                ("PREFER_LOWER_GRADE", false),
            ]),
        )
        .unwrap_err();
        assert!(e.user_message.contains("하나 이상"), "{}", e.user_message);

        // 막혔으니 기존 설정이 그대로여야 한다
        assert_eq!(order_of(&c), vec!["SAME_GRADE", "FEWEST_TODAY", "FEWEST_TOTAL"]);
    }

    #[test]
    fn 모르는_기준_이름은_저장을_막는다() {
        let c = memory_conn();
        let e = save(&c, &input(&[("옛날기준", true)])).unwrap_err();
        assert!(e.user_message.contains("알 수 없는"), "{}", e.user_message);
    }

    #[test]
    fn 빈_목록은_저장을_막는다() {
        let c = memory_conn();
        assert!(save(&c, &[]).is_err());
    }

    #[test]
    fn 옛_버전이_남긴_모르는_기준은_무시한다() {
        let c = memory_conn();
        // 001 기본 데이터에 있던 SAME_SUBJECT / PREFER_ADJACENT 는 아직 구현되지 않았다
        let raw: i64 = c
            .query_row("SELECT COUNT(*) FROM priority_rules", [], |r| r.get(0))
            .unwrap();
        assert!(raw >= 7);

        let known = settings(&c).unwrap();
        assert!(
            known.iter().all(|s| is_known_key(&s.rule_key)),
            "프로그램이 아는 기준만 넘어와야 한다"
        );
        assert_eq!(view(&c).unwrap().rules.len(), 7);
    }
}

