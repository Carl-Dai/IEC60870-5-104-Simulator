use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Direction of the IEC 104 communication frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Received (inbound)
    Rx,
    /// Sent (outbound)
    Tx,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Rx => write!(f, "RX"),
            Direction::Tx => write!(f, "TX"),
        }
    }
}

/// IEC 104 frame type label for logging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameLabel {
    /// I-frame with ASDU type name
    IFrame(String),
    /// S-frame (supervisory)
    SFrame,
    /// U-frame: STARTDT ACT
    UStartAct,
    /// U-frame: STARTDT CON
    UStartCon,
    /// U-frame: STOPDT ACT
    UStopAct,
    /// U-frame: STOPDT CON
    UStopCon,
    /// U-frame: TESTFR ACT
    UTestAct,
    /// U-frame: TESTFR CON
    UTestCon,
    /// General interrogation
    GeneralInterrogation,
    /// Counter interrogation
    CounterRead,
    /// Clock synchronization
    ClockSync,
    /// Single command
    SingleCommand,
    /// Double command
    DoubleCommand,
    /// Setpoint normalized
    SetpointNormalized,
    /// Setpoint scaled
    SetpointScaled,
    /// Setpoint float
    SetpointFloat,
    /// Connection event
    ConnectionEvent,
}

impl FrameLabel {
    pub fn name(&self) -> String {
        match self {
            Self::IFrame(asdu) => format!("I {}", asdu),
            Self::SFrame => "S".to_string(),
            Self::UStartAct => "U STARTDT ACT".to_string(),
            Self::UStartCon => "U STARTDT CON".to_string(),
            Self::UStopAct => "U STOPDT ACT".to_string(),
            Self::UStopCon => "U STOPDT CON".to_string(),
            Self::UTestAct => "U TESTFR ACT".to_string(),
            Self::UTestCon => "U TESTFR CON".to_string(),
            Self::GeneralInterrogation => "GI".to_string(),
            Self::CounterRead => "CI".to_string(),
            Self::ClockSync => "CS".to_string(),
            Self::SingleCommand => "C_SC".to_string(),
            Self::DoubleCommand => "C_DC".to_string(),
            Self::SetpointNormalized => "C_SE_NA".to_string(),
            Self::SetpointScaled => "C_SE_NB".to_string(),
            Self::SetpointFloat => "C_SE_NC".to_string(),
            Self::ConnectionEvent => "CONN".to_string(),
        }
    }
}

/// A single entry in the communication log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp when the frame was captured.
    pub timestamp: DateTime<Utc>,
    /// Direction: received or sent.
    pub direction: Direction,
    /// Frame type label.
    pub frame_label: FrameLabel,
    /// Human-readable detail description.
    pub detail: String,
    /// Raw bytes of the frame (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_bytes: Option<Vec<u8>>,
}

impl LogEntry {
    /// Create a new log entry with the current timestamp.
    pub fn new(direction: Direction, frame_label: FrameLabel, detail: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            direction,
            frame_label,
            detail: detail.into(),
            raw_bytes: None,
        }
    }

    /// Create a new log entry with raw bytes included.
    pub fn with_raw_bytes(
        direction: Direction,
        frame_label: FrameLabel,
        detail: impl Into<String>,
        raw_bytes: Vec<u8>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            direction,
            frame_label,
            detail: detail.into(),
            raw_bytes: Some(raw_bytes),
        }
    }

    /// Format for CSV export.
    pub fn to_csv_row(&self) -> String {
        let timestamp = self.timestamp.format("%Y-%m-%d %H:%M:%S%.3f");
        let direction = self.direction.to_string();
        let label = self.frame_label.name();
        let raw = self.raw_bytes.as_ref()
            .map(|b| b.iter().map(|v| format!("{:02X}", v)).collect::<Vec<_>>().join(" "))
            .unwrap_or_default();
        format!("\"{}\",{},{},\"{}\",\"{}\"", timestamp, direction, label, self.detail, raw)
    }

    /// CSV header row.
    pub fn csv_header() -> &'static str {
        "Timestamp,Direction,FrameType,Detail,RawBytes"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::new(Direction::Rx, FrameLabel::GeneralInterrogation, "GI CA=1");
        assert_eq!(entry.direction, Direction::Rx);
        assert_eq!(entry.frame_label, FrameLabel::GeneralInterrogation);
        assert_eq!(entry.detail, "GI CA=1");
        assert!(entry.raw_bytes.is_none());
    }

    #[test]
    fn test_frame_label_name() {
        assert_eq!(FrameLabel::IFrame("M_SP_NA_1".to_string()).name(), "I M_SP_NA_1");
        assert_eq!(FrameLabel::SFrame.name(), "S");
        assert_eq!(FrameLabel::UStartAct.name(), "U STARTDT ACT");
        assert_eq!(FrameLabel::GeneralInterrogation.name(), "GI");
    }

    #[test]
    fn test_csv_export() {
        let entry = LogEntry::new(Direction::Tx, FrameLabel::GeneralInterrogation, "GI CA=1");
        let row = entry.to_csv_row();
        assert!(row.contains("TX"));
        assert!(row.contains("GI"));
        assert!(row.contains("GI CA=1"));
    }
}
