//! Thesis 鈥?鍙拷韪殑鍒ゆ柇
//!
//! Thesis 鏄?鎴戣涓轰細鍙戠敓浠€涔?鐨勫叿浣撳垽鏂€?
//! 瀹冩槸绯荤粺鏍稿績璁ょ煡閾捐矾涓殑鍏抽敭浜х墿锛?
//!   - 鍙獙璇侊紙鍦ㄥ皢鏉ユ湁鏄庣‘鐨?"瀵?閿? 鍒ゅ畾锛?
//!   - 鏈夋椂闂磋竟鐣?
//!   - 鏈夌疆淇″害
//!
//! 濂戠害杈圭晫锛?
//!   Producer: Intelligence Engine (Thesis Generation step)
//!   Consumer: Intelligence Engine (Decision Mapping step)
//!             Memory (杩借釜銆侀獙璇併€佸弽鎬?

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 鍙拷韪垽鏂?鈥?绯荤粺璁ょ煡妯″瀷鐨勬牳蹇冧骇鐗?
///
/// # 璁捐鍘熷垯
/// - Thesis 蹇呴』鍙瘉浼紙falsification_conditions 鏄庣‘鍐欏嚭"浠€涔堟儏鍐典笅鎴戦敊浜?锛?
/// - time_horizon 缁欏嚭鍒ゆ柇鐨勬湁鏁堟湡锛堣繃浜嗚繖涓椂闂存病鏈夌粨鏋?= 鑷姩 Pending锛?
/// - evidence 鏄?Signal ID 鍒楄〃锛屽舰鎴愬畬鏁磋瘉鎹摼
/// - theme 鍜?belief_statement 鏄?Phase 2 鍐呴儴瀛楁锛屾殏涓嶅疄浣撳寲涓虹嫭绔嬫楠?
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Thesis {
    /// 鍞竴 ID锛屾牸寮?"thesis_xxx"
    pub id: String,

    /// 鍒ゆ柇闄堣堪锛堝 "GPU supply chain will tighten further in 2026 Q4"锛?
    pub claim: String,

    /// 褰撳墠缃俊搴?[0.0, 1.0]
    pub confidence: f64,

    /// 鏀寔璇佹嵁锛圫ignal ID 鍒楄〃锛?
    #[serde(default)]
    pub evidence: Vec<String>,

    /// Thesis 鐘舵€?
    pub status: ThesisStatus,

    /// 璇佷吉鏉′欢 鈥?浠€涔堟儏鍐典笅杩欎釜鍒ゆ柇鏄敊璇殑
    /// 渚嬪: ["浼佷笟 AI Agent 閲囩敤鐜囪繛缁?12 涓湀娌℃湁澧為暱"]
    /// 杩欐槸 Reflection 鍒ゆ柇"褰撴椂鎴戝埌搴曢娴嬩簡浠€涔?鐨勫叧閿瓧娈?
    #[serde(default)]
    pub falsification_conditions: Vec<String>,

    /// 鍒ゆ柇鏈夋晥鏈?鈥?濡?"12_months", "6_months", "30_days"
    /// 鍒版湡鍚庤嚜鍔ㄦ爣璁颁负 Pending锛岀瓑寰?Outcome 纭/璇佷吉
    #[serde(default = "default_time_horizon")]
    pub time_horizon: String,

    /// 涓婚鍚嶏紙Phase 2 鍐呴儴瀛楁锛屾殏涓嶅疄浣撳寲涓?Theme 姝ラ锛?
    #[serde(default)]
    pub theme: Option<String>,

    /// 淇″康闄堣堪锛圥hase 2 鍐呴儴瀛楁锛屾殏涓嶅疄浣撳寲涓?Belief 姝ラ锛?
    #[serde(default)]
    pub belief_statement: Option<String>,

    /// 鎽樿锛圠LM 鐢熸垚鐨勭畝鐭€荤粨锛岀敤浜庡墠绔睍绀猴級
    /// 涓虹┖鏃跺洖閫€浣跨敤 claim 浣滀负鎽樿
    #[serde(default)]
    pub summary: Option<String>,
}

fn default_time_horizon() -> String {
    "12_months".into()
}

/// Thesis 鐢熷懡鍛ㄦ湡鐘舵€?
///
/// # 鐘舵€佹槧灏勶紙杈撳嚭鍒板墠绔椂锛?
/// Rust 鍐呴儴浣跨敤瀹屾暣璇箟锛堝寘鎷?Pending/Confirmed/Invalidated锛夛紝
/// 杈撳嚭鍒?MDX 鍓嶇鏃舵槧灏勪负 frontend schema 鍏煎鍊硷細
///   - Pending     鈫?"dormant"锛堟棤杩戞湡娲诲姩锛?
///   - Confirmed   鈫?"active"锛堝凡楠岃瘉涓虹湡锛屼粛娲昏穬锛?
///   - Invalidated 鈫?"retired"锛堝凡璇佷吉锛屼笉鍐嶈拷韪級
///
/// 鍓嶇 schema锛堣 intel-web/src/content/config.ts锛?
///   proposed | active | strengthening | weakening | dormant | retired
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub enum ThesisStatus {
    /// 鏂板缓鎻愭
    #[default]
    Proposed,
    /// 甯歌璺熻釜
    Active,
    /// 杩戞湡鏈夊己鍖栦俊鍙?
    Strengthening,
    /// 杩戞湡鏈夋寫鎴樹俊鍙?
    Weakening,
    /// 寰呴獙璇侊紙鍒版湡鍙‘璁?璇佷吉锛?
    Pending,
    /// 宸茬‘璁わ紙Outcome 楠岃瘉涓虹湡锛?
    Confirmed,
    /// 宸茶瘉浼紙Outcome 楠岃瘉涓哄亣锛?
    Invalidated,
    /// 浼戠湢锛堟棤杩戞湡娲诲姩淇″彿锛?
    Dormant,
    /// 宸插綊妗ｉ€€褰?
    Retired,
}

impl ThesisStatus {
    /// 鏄犲皠涓哄墠绔?schema 鍏煎鐨勫瓧绗︿覆鍊?
    /// 鐢ㄤ簬 MDX frontmatter 杈撳嚭
    pub fn to_frontend_string(&self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Active => "active",
            Self::Strengthening => "strengthening",
            Self::Weakening => "weakening",
            Self::Pending => "dormant",
            Self::Confirmed => "active",
            Self::Invalidated => "retired",
            Self::Dormant => "dormant",
            Self::Retired => "retired",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thesis_serde_roundtrip() {
        let thesis = Thesis {
            id: "thesis_001".into(),
            claim: "AI Agent adoption will accelerate".into(),
            confidence: 0.72,
            evidence: vec!["sig_001".into()],
            status: ThesisStatus::Active,
            falsification_conditions: vec!["Adoption flat for 12mo".into()],
            time_horizon: "12_months".into(),
            theme: Some("AI Enterprise".into()),
            belief_statement: None,
            summary: None,
        };
        let json = serde_json::to_string(&thesis).unwrap();
        let restored: Thesis = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, thesis.id);
        assert!(matches!(restored.status, ThesisStatus::Active));
        assert_eq!(restored.falsification_conditions.len(), 1);
    }

    #[test]
    fn test_thesis_default_evidence_empty() {
        let json = r#"{"id":"t1","claim":"test","confidence":0.5,"status":"Proposed","time_horizon":"12_months"}"#;
        let thesis: Thesis = serde_json::from_str(json).unwrap();
        assert!(thesis.evidence.is_empty());
    }
}

