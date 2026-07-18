//! Length-prefixed bincode helpers. Wire format: `u32 LE length | bincode payload`.
//! See `06_tech-design/03-ipc-protocol.md` §6.

use crate::ProtocolError;

pub const MAX_MSG_BYTES: usize = 4 * 1024 * 1024;

pub fn encode_message<T: serde::Serialize>(msg: &T) -> Result<Vec<u8>, ProtocolError> {
    let payload = bincode::serde::encode_to_vec(msg, bincode::config::standard())
        .map_err(|e| ProtocolError::DecodeFailed(e.to_string()))?;
    let len = payload.len();
    if len > MAX_MSG_BYTES {
        return Err(ProtocolError::MessageTooLarge(len, MAX_MSG_BYTES));
    }
    let mut out = Vec::with_capacity(4 + len);
    out.extend_from_slice(&(len as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

pub fn decode_message<T: serde::de::DeserializeOwned>(
    payload: &[u8],
) -> Result<(T, usize), ProtocolError> {
    bincode::serde::decode_from_slice(payload, bincode::config::standard())
        .map_err(|e| ProtocolError::DecodeFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClientMessage, ServerEvent};

    #[test]
    fn client_message_roundtrip() {
        let msg = ClientMessage::RequestFullState;
        let bytes = encode_message(&msg).unwrap();
        assert!(bytes.len() > 4);
        let payload = &bytes[4..];
        let (decoded, _): (ClientMessage, usize) = decode_message(payload).unwrap();
        assert!(matches!(decoded, ClientMessage::RequestFullState));
    }

    #[test]
    fn pane_input_roundtrip() {
        use crate::{PaneId, TabId};
        let msg = ClientMessage::PaneInput {
            tab_id: TabId(1),
            pane_id: PaneId(1),
            data: vec![b'a'],
        };
        let bytes = encode_message(&msg).unwrap();
        let payload = &bytes[4..];
        let (decoded, _): (ClientMessage, usize) = decode_message(payload).unwrap();
        match decoded {
            ClientMessage::PaneInput {
                tab_id,
                pane_id,
                data,
            } => {
                assert_eq!(tab_id, TabId(1));
                assert_eq!(pane_id, PaneId(1));
                assert_eq!(data, vec![b'a']);
            }
            other => panic!("expected PaneInput, got {other:?}"),
        }
    }

    #[test]
    fn copy_to_clipboard_roundtrip() {
        let msg = ClientMessage::CopyToClipboard {
            text: "hello world".to_string(),
        };
        let bytes = encode_message(&msg).unwrap();
        let (decoded, _): (ClientMessage, _) =
            bincode::serde::decode_from_slice(&bytes[4..], bincode::config::standard()).unwrap();
        match decoded {
            ClientMessage::CopyToClipboard { text } => assert_eq!(text, "hello world"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn server_event_roundtrip() {
        let msg = ServerEvent::Ping;
        let bytes = encode_message(&msg).unwrap();
        let payload = &bytes[4..];
        let (decoded, _): (ServerEvent, usize) = decode_message(payload).unwrap();
        assert!(matches!(decoded, ServerEvent::Ping));
    }

    #[test]
    fn resize_split_roundtrip() {
        use crate::{PaneId, TabId};
        let msg = ClientMessage::ResizeSplit {
            tab_id: TabId(1),
            first_pane: PaneId(2),
            second_pane: PaneId(3),
            ratio: 0.35,
        };
        let bytes = encode_message(&msg).unwrap();
        let payload = &bytes[4..];
        let (decoded, _): (ClientMessage, usize) = decode_message(payload).unwrap();
        match decoded {
            ClientMessage::ResizeSplit {
                tab_id,
                first_pane,
                second_pane,
                ratio,
            } => {
                assert_eq!(tab_id, TabId(1));
                assert_eq!(first_pane, PaneId(2));
                assert_eq!(second_pane, PaneId(3));
                assert!((ratio - 0.35).abs() < f32::EPSILON);
            }
            other => panic!("expected ResizeSplit, got {other:?}"),
        }
    }
}
