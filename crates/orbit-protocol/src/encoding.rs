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
    fn server_event_roundtrip() {
        let msg = ServerEvent::Ping;
        let bytes = encode_message(&msg).unwrap();
        let payload = &bytes[4..];
        let (decoded, _): (ServerEvent, usize) = decode_message(payload).unwrap();
        assert!(matches!(decoded, ServerEvent::Ping));
    }
}
