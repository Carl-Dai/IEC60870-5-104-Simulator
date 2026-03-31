use serde::{Deserialize, Serialize};

/// IEC 60870-5-101/104 ASDU Type Identifiers (commonly used subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsduTypeId {
    // ---- Monitor direction (slave -> master) ----
    /// Single-point information (Type 1)
    MSpNa1 = 1,
    /// Single-point with CP56Time2a (Type 30)
    MSpTb1 = 30,
    /// Double-point information (Type 3)
    MDpNa1 = 3,
    /// Double-point with CP56Time2a (Type 31)
    MDpTb1 = 31,
    /// Step position (Type 5)
    MStNa1 = 5,
    /// Step position with CP56Time2a (Type 32)
    MStTb1 = 32,
    /// Bitstring of 32 bit (Type 7)
    MBoNa1 = 7,
    /// Bitstring with CP56Time2a (Type 33)
    MBoTb1 = 33,
    /// Measured value, normalized (Type 9)
    MMeNa1 = 9,
    /// Measured value, normalized with CP56Time2a (Type 34)
    MMeTd1 = 34,
    /// Measured value, scaled (Type 11)
    MMeNb1 = 11,
    /// Measured value, scaled with CP56Time2a (Type 35)
    MMeTe1 = 35,
    /// Measured value, short floating point (Type 13)
    MMeNc1 = 13,
    /// Measured value, short float with CP56Time2a (Type 36)
    MMeTf1 = 36,
    /// Integrated totals (Type 15)
    MItNa1 = 15,
    /// Integrated totals with CP56Time2a (Type 37)
    MItTb1 = 37,

    // ---- Control direction (master -> slave) ----
    /// Single command (Type 45)
    CScNa1 = 45,
    /// Double command (Type 46)
    CDcNa1 = 46,
    /// Set-point, normalized (Type 48)
    CSeNa1 = 48,
    /// Set-point, scaled (Type 49)
    CSeNb1 = 49,
    /// Set-point, short floating point (Type 50)
    CSeNc1 = 50,

    // ---- System commands ----
    /// Interrogation command (Type 100)
    CIcNa1 = 100,
    /// Counter interrogation command (Type 101)
    CCiNa1 = 101,
    /// Clock synchronization command (Type 103)
    CCsNa1 = 103,
}

impl AsduTypeId {
    /// Short display name for ASDU type.
    pub fn name(&self) -> &'static str {
        match self {
            Self::MSpNa1 => "M_SP_NA_1",
            Self::MSpTb1 => "M_SP_TB_1",
            Self::MDpNa1 => "M_DP_NA_1",
            Self::MDpTb1 => "M_DP_TB_1",
            Self::MStNa1 => "M_ST_NA_1",
            Self::MStTb1 => "M_ST_TB_1",
            Self::MBoNa1 => "M_BO_NA_1",
            Self::MBoTb1 => "M_BO_TB_1",
            Self::MMeNa1 => "M_ME_NA_1",
            Self::MMeTd1 => "M_ME_TD_1",
            Self::MMeNb1 => "M_ME_NB_1",
            Self::MMeTe1 => "M_ME_TE_1",
            Self::MMeNc1 => "M_ME_NC_1",
            Self::MMeTf1 => "M_ME_TF_1",
            Self::MItNa1 => "M_IT_NA_1",
            Self::MItTb1 => "M_IT_TB_1",
            Self::CScNa1 => "C_SC_NA_1",
            Self::CDcNa1 => "C_DC_NA_1",
            Self::CSeNa1 => "C_SE_NA_1",
            Self::CSeNb1 => "C_SE_NB_1",
            Self::CSeNc1 => "C_SE_NC_1",
            Self::CIcNa1 => "C_IC_NA_1",
            Self::CCiNa1 => "C_CI_NA_1",
            Self::CCsNa1 => "C_CS_NA_1",
        }
    }

    /// Human-readable Chinese description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::MSpNa1 | Self::MSpTb1 => "单点信息",
            Self::MDpNa1 | Self::MDpTb1 => "双点信息",
            Self::MStNa1 | Self::MStTb1 => "步位置信息",
            Self::MBoNa1 | Self::MBoTb1 => "32位串",
            Self::MMeNa1 | Self::MMeTd1 => "归一化测量值",
            Self::MMeNb1 | Self::MMeTe1 => "标度化测量值",
            Self::MMeNc1 | Self::MMeTf1 => "短浮点测量值",
            Self::MItNa1 | Self::MItTb1 => "累计量",
            Self::CScNa1 => "单点命令",
            Self::CDcNa1 => "双点命令",
            Self::CSeNa1 => "归一化设定值",
            Self::CSeNb1 => "标度化设定值",
            Self::CSeNc1 => "短浮点设定值",
            Self::CIcNa1 => "总召唤",
            Self::CCiNa1 => "累计量召唤",
            Self::CCsNa1 => "时钟同步",
        }
    }

    /// Get the data category this ASDU type belongs to.
    pub fn category(&self) -> DataCategory {
        match self {
            Self::MSpNa1 | Self::MSpTb1 => DataCategory::SinglePoint,
            Self::MDpNa1 | Self::MDpTb1 => DataCategory::DoublePoint,
            Self::MStNa1 | Self::MStTb1 => DataCategory::StepPosition,
            Self::MBoNa1 | Self::MBoTb1 => DataCategory::Bitstring,
            Self::MMeNa1 | Self::MMeTd1 => DataCategory::NormalizedMeasured,
            Self::MMeNb1 | Self::MMeTe1 => DataCategory::ScaledMeasured,
            Self::MMeNc1 | Self::MMeTf1 => DataCategory::FloatMeasured,
            Self::MItNa1 | Self::MItTb1 => DataCategory::IntegratedTotals,
            Self::CScNa1 => DataCategory::SinglePoint,
            Self::CDcNa1 => DataCategory::DoublePoint,
            Self::CSeNa1 => DataCategory::NormalizedMeasured,
            Self::CSeNb1 => DataCategory::ScaledMeasured,
            Self::CSeNc1 => DataCategory::FloatMeasured,
            Self::CIcNa1 | Self::CCiNa1 | Self::CCsNa1 => DataCategory::System,
        }
    }

    /// Parse from type ID integer.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::MSpNa1),
            3 => Some(Self::MDpNa1),
            5 => Some(Self::MStNa1),
            7 => Some(Self::MBoNa1),
            9 => Some(Self::MMeNa1),
            11 => Some(Self::MMeNb1),
            13 => Some(Self::MMeNc1),
            15 => Some(Self::MItNa1),
            30 => Some(Self::MSpTb1),
            31 => Some(Self::MDpTb1),
            32 => Some(Self::MStTb1),
            33 => Some(Self::MBoTb1),
            34 => Some(Self::MMeTd1),
            35 => Some(Self::MMeTe1),
            36 => Some(Self::MMeTf1),
            37 => Some(Self::MItTb1),
            45 => Some(Self::CScNa1),
            46 => Some(Self::CDcNa1),
            48 => Some(Self::CSeNa1),
            49 => Some(Self::CSeNb1),
            50 => Some(Self::CSeNc1),
            100 => Some(Self::CIcNa1),
            101 => Some(Self::CCiNa1),
            103 => Some(Self::CCsNa1),
            _ => None,
        }
    }
}

/// Data categories for grouping ASDU types in the UI tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataCategory {
    /// M_SP_* (single-point information)
    SinglePoint,
    /// M_DP_* (double-point information)
    DoublePoint,
    /// M_ST_* (step position)
    StepPosition,
    /// M_BO_* (bitstring)
    Bitstring,
    /// M_ME_NA/TD (normalized measured value)
    NormalizedMeasured,
    /// M_ME_NB/TE (scaled measured value)
    ScaledMeasured,
    /// M_ME_NC/TF (short floating point measured value)
    FloatMeasured,
    /// M_IT_* (integrated totals / counters)
    IntegratedTotals,
    /// System commands (GI, Counter Read, Clock Sync)
    System,
}

impl DataCategory {
    /// Short display name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::SinglePoint => "单点 (SP)",
            Self::DoublePoint => "双点 (DP)",
            Self::StepPosition => "步位置 (ST)",
            Self::Bitstring => "位串 (BO)",
            Self::NormalizedMeasured => "归一化 (ME_NA)",
            Self::ScaledMeasured => "标度化 (ME_NB)",
            Self::FloatMeasured => "浮点 (ME_NC)",
            Self::IntegratedTotals => "累计量 (IT)",
            Self::System => "系统命令",
        }
    }

    /// All monitor-direction categories (for tree display).
    pub fn monitor_categories() -> &'static [DataCategory] {
        &[
            Self::SinglePoint,
            Self::DoublePoint,
            Self::StepPosition,
            Self::Bitstring,
            Self::NormalizedMeasured,
            Self::ScaledMeasured,
            Self::FloatMeasured,
            Self::IntegratedTotals,
        ]
    }
}

/// Quality descriptor flags per IEC 60870-5-101.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct QualityFlags {
    /// Overflow
    pub ov: bool,
    /// Blocked
    pub bl: bool,
    /// Substituted
    pub sb: bool,
    /// Not topical
    pub nt: bool,
    /// Invalid
    pub iv: bool,
}

impl QualityFlags {
    pub fn good() -> Self {
        Self::default()
    }

    pub fn invalid() -> Self {
        Self { iv: true, ..Default::default() }
    }
}

/// Cause of Transmission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CauseOfTransmission {
    Periodic = 1,
    Background = 2,
    Spontaneous = 3,
    Initialized = 4,
    Request = 5,
    Activation = 6,
    ActivationCon = 7,
    Deactivation = 8,
    DeactivationCon = 9,
    ActivationTermination = 10,
    Interrogated = 20,
    CounterInterrogated = 37,
}

impl CauseOfTransmission {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Periodic),
            2 => Some(Self::Background),
            3 => Some(Self::Spontaneous),
            4 => Some(Self::Initialized),
            5 => Some(Self::Request),
            6 => Some(Self::Activation),
            7 => Some(Self::ActivationCon),
            8 => Some(Self::Deactivation),
            9 => Some(Self::DeactivationCon),
            10 => Some(Self::ActivationTermination),
            20 => Some(Self::Interrogated),
            37 => Some(Self::CounterInterrogated),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Periodic => "周期",
            Self::Background => "背景",
            Self::Spontaneous => "突发",
            Self::Initialized => "初始化",
            Self::Request => "请求",
            Self::Activation => "激活",
            Self::ActivationCon => "激活确认",
            Self::Deactivation => "停止激活",
            Self::DeactivationCon => "停止确认",
            Self::ActivationTermination => "激活终止",
            Self::Interrogated => "总召唤",
            Self::CounterInterrogated => "累计量召唤",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asdu_type_from_u8() {
        assert_eq!(AsduTypeId::from_u8(1), Some(AsduTypeId::MSpNa1));
        assert_eq!(AsduTypeId::from_u8(13), Some(AsduTypeId::MMeNc1));
        assert_eq!(AsduTypeId::from_u8(100), Some(AsduTypeId::CIcNa1));
        assert_eq!(AsduTypeId::from_u8(255), None);
    }

    #[test]
    fn test_asdu_type_category() {
        assert_eq!(AsduTypeId::MSpNa1.category(), DataCategory::SinglePoint);
        assert_eq!(AsduTypeId::MSpTb1.category(), DataCategory::SinglePoint);
        assert_eq!(AsduTypeId::MMeNc1.category(), DataCategory::FloatMeasured);
        assert_eq!(AsduTypeId::MItNa1.category(), DataCategory::IntegratedTotals);
    }

    #[test]
    fn test_quality_flags_default() {
        let q = QualityFlags::good();
        assert!(!q.ov && !q.bl && !q.sb && !q.nt && !q.iv);
    }

    #[test]
    fn test_asdu_type_name() {
        assert_eq!(AsduTypeId::MSpNa1.name(), "M_SP_NA_1");
        assert_eq!(AsduTypeId::CIcNa1.name(), "C_IC_NA_1");
    }
}
