use serde::{Deserialize, Serialize};

/// IEC 104 APCI frame types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FrameType {
    /// I-frame (Information transfer)
    IFrame {
        send_seq: u16,
        recv_seq: u16,
        asdu_type: u8,
        cause: u8,
        common_address: u16,
    },
    /// S-frame (Supervisory)
    SFrame {
        recv_seq: u16,
    },
    /// U-frame (Unnumbered)
    UFrame {
        kind: UFrameKind,
    },
}

/// U-frame subtypes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UFrameKind {
    StartDtAct,
    StartDtCon,
    StopDtAct,
    StopDtCon,
    TestFrAct,
    TestFrCon,
}

impl UFrameKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::StartDtAct => "STARTDT ACT",
            Self::StartDtCon => "STARTDT CON",
            Self::StopDtAct => "STOPDT ACT",
            Self::StopDtCon => "STOPDT CON",
            Self::TestFrAct => "TESTFR ACT",
            Self::TestFrCon => "TESTFR CON",
        }
    }
}

/// Parse an APCI frame from raw bytes.
///
/// IEC 104 frame format:
/// - Byte 0: Start byte (0x68)
/// - Byte 1: Length (of remaining bytes)
/// - Bytes 2-5: Control fields (determine frame type)
/// - Bytes 6+: ASDU (for I-frames only)
pub fn parse_apci(data: &[u8]) -> Result<FrameType, FrameError> {
    if data.len() < 6 {
        return Err(FrameError::TooShort);
    }

    if data[0] != 0x68 {
        return Err(FrameError::InvalidStartByte(data[0]));
    }

    let ctrl1 = data[2];
    let ctrl2 = data[3];
    let ctrl3 = data[4];
    let ctrl4 = data[5];

    // Determine frame type from control field byte 1
    if ctrl1 & 0x01 == 0 {
        // I-frame: bit 0 of ctrl1 is 0
        let send_seq = ((ctrl1 as u16) >> 1) | ((ctrl2 as u16) << 7);
        let recv_seq = ((ctrl3 as u16) >> 1) | ((ctrl4 as u16) << 7);

        // Parse ASDU if present
        let (asdu_type, cause, common_address) = if data.len() >= 10 {
            let asdu_type = data[6];
            let cause = data[8];
            let ca = u16::from_le_bytes([data[10.min(data.len() - 1)], data[11.min(data.len() - 1)]]);
            (asdu_type, cause, ca)
        } else {
            (0, 0, 0)
        };

        Ok(FrameType::IFrame {
            send_seq,
            recv_seq,
            asdu_type,
            cause,
            common_address,
        })
    } else if ctrl1 & 0x03 == 0x01 {
        // S-frame: bits 0-1 of ctrl1 are 01
        let recv_seq = ((ctrl3 as u16) >> 1) | ((ctrl4 as u16) << 7);
        Ok(FrameType::SFrame { recv_seq })
    } else if ctrl1 & 0x03 == 0x03 {
        // U-frame: bits 0-1 of ctrl1 are 11
        let kind = match ctrl1 {
            0x07 => UFrameKind::StartDtAct,
            0x0B => UFrameKind::StartDtCon,
            0x13 => UFrameKind::StopDtAct,
            0x23 => UFrameKind::StopDtCon,
            0x43 => UFrameKind::TestFrAct,
            0x83 => UFrameKind::TestFrCon,
            _ => return Err(FrameError::UnknownUFrame(ctrl1)),
        };
        Ok(FrameType::UFrame { kind })
    } else {
        Err(FrameError::UnknownFrameType(ctrl1))
    }
}

/// Format a frame summary for display.
pub fn format_frame_summary(frame: &FrameType) -> String {
    match frame {
        FrameType::IFrame { send_seq, recv_seq, asdu_type, cause, common_address } => {
            let type_name = crate::types::AsduTypeId::from_u8(*asdu_type)
                .map(|t| t.name().to_string())
                .unwrap_or_else(|| format!("Type{}", asdu_type));
            let cause_name = crate::types::CauseOfTransmission::from_u8(*cause)
                .map(|c| c.name().to_string())
                .unwrap_or_else(|| format!("CoT{}", cause));
            format!("I [S={} R={}] {} {} CA={}", send_seq, recv_seq, type_name, cause_name, common_address)
        }
        FrameType::SFrame { recv_seq } => {
            format!("S [R={}]", recv_seq)
        }
        FrameType::UFrame { kind } => {
            format!("U {}", kind.name())
        }
    }
}

/// Format raw bytes as hex string.
pub fn format_hex(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame too short (need at least 6 bytes)")]
    TooShort,
    #[error("invalid start byte: 0x{0:02X} (expected 0x68)")]
    InvalidStartByte(u8),
    #[error("unknown U-frame type: 0x{0:02X}")]
    UnknownUFrame(u8),
    #[error("unknown frame type: 0x{0:02X}")]
    UnknownFrameType(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_u_frame_startdt_act() {
        let data = [0x68, 0x04, 0x07, 0x00, 0x00, 0x00];
        let frame = parse_apci(&data).unwrap();
        assert_eq!(frame, FrameType::UFrame { kind: UFrameKind::StartDtAct });
    }

    #[test]
    fn test_parse_u_frame_startdt_con() {
        let data = [0x68, 0x04, 0x0B, 0x00, 0x00, 0x00];
        let frame = parse_apci(&data).unwrap();
        assert_eq!(frame, FrameType::UFrame { kind: UFrameKind::StartDtCon });
    }

    #[test]
    fn test_parse_s_frame() {
        // S-frame with recv_seq=5: ctrl3=0x0A (5<<1), ctrl4=0x00
        let data = [0x68, 0x04, 0x01, 0x00, 0x0A, 0x00];
        let frame = parse_apci(&data).unwrap();
        assert_eq!(frame, FrameType::SFrame { recv_seq: 5 });
    }

    #[test]
    fn test_parse_too_short() {
        let data = [0x68, 0x04, 0x07];
        assert!(parse_apci(&data).is_err());
    }

    #[test]
    fn test_format_hex() {
        assert_eq!(format_hex(&[0x68, 0x04, 0x07, 0x00]), "68 04 07 00");
    }
}
