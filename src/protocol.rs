use bytes::{Buf, BufMut, Bytes, BytesMut};
use lexpr::Value;
use tracing::{debug, warn};

/// EPC Protocol message enum
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// Call a remote method: (call uid method-name args)
    Call {
        uid: u64,
        method: String,
        args: Value,
    },

    /// Return a value: (return uid result)
    Return { uid: u64, result: Value },

    /// Return an application error: (return-error uid error-message)
    ReturnError { uid: u64, error: String },

    /// Return a protocol error: (epc-error uid error-message)
    EPCError { uid: u64, error: String },

    /// Query available methods: (methods uid)
    Methods { uid: u64 },
}

impl Message {
    /// Create a new call message
    pub fn new_call(uid: u64, method: impl Into<String>, args: Value) -> Self {
        Message::Call {
            uid,
            method: method.into(),
            args,
        }
    }

    /// Create a new return message
    pub fn new_return(uid: u64, result: Value) -> Self {
        Message::Return { uid, result }
    }

    /// Create a new return-error message
    pub fn new_return_error(uid: u64, error: impl Into<String>) -> Self {
        Message::ReturnError {
            uid,
            error: error.into(),
        }
    }

    /// Create a new epc-error message
    pub fn new_epc_error(uid: u64, error: impl Into<String>) -> Self {
        Message::EPCError {
            uid,
            error: error.into(),
        }
    }

    /// Create a new methods query message
    pub fn new_methods(uid: u64) -> Self {
        Message::Methods { uid }
    }

    /// Get the UID of the message
    pub fn uid(&self) -> u64 {
        match self {
            Message::Call { uid, .. } => *uid,
            Message::Return { uid, .. } => *uid,
            Message::ReturnError { uid, .. } => *uid,
            Message::EPCError { uid, .. } => *uid,
            Message::Methods { uid } => *uid,
        }
    }

    /// Serialize message to S-expression string
    pub fn to_sexp(&self) -> std::result::Result<String, crate::error::ERPCError> {
        debug!("Serializing message: {:?}", self);
        let sexp = match self {
            Message::Call { uid, method, args } => {
                debug!(
                    "Serializing CALL uid={}, method={}, args={:?}",
                    uid, method, args
                );
                Value::list(vec![
                    Value::symbol("call"),
                    Value::from(*uid as i64),
                    Value::symbol(method.clone()),
                    args.clone(),
                ])
            }
            Message::Return { uid, result } => {
                debug!("Serializing RETURN uid={}, result={:?}", uid, result);
                Value::list(vec![
                    Value::symbol("return"),
                    Value::from(*uid as i64),
                    result.clone(),
                ])
            }
            Message::ReturnError { uid, error } => {
                debug!("Serializing RETURN-ERROR uid={}, error={}", uid, error);
                Value::list(vec![
                    Value::symbol("return-error"),
                    Value::from(*uid as i64),
                    Value::string(error.clone()),
                ])
            }
            Message::EPCError { uid, error } => {
                debug!("Serializing EPC-ERROR uid={}, error={}", uid, error);
                Value::list(vec![
                    Value::symbol("epc-error"),
                    Value::from(*uid as i64),
                    Value::string(error.clone()),
                ])
            }
            Message::Methods { uid } => {
                debug!("Serializing METHODS uid={}", uid);
                Value::list(vec![Value::symbol("methods"), Value::from(*uid as i64)])
            }
        };

        let result = lexpr::to_string(&sexp)
            .map_err(|e| crate::error::ERPCError::SerializationError(e.to_string()));
        debug!(
            "Serialized to: {}",
            result.as_ref().unwrap_or(&"ERROR".to_string())
        );
        result
    }

    /// Parse message from S-expression string
    pub fn from_sexp(s: &str) -> std::result::Result<Self, crate::error::ERPCError> {
        debug!("Parsing S-expression: {}", s);
        let value = lexpr::from_str(s)?;

        debug!("Parsed value: {:?}", value);

        // Handle both Cons and proper list formats
        let items: Vec<Value> = match value {
            Value::Cons(cons) => {
                let items: Vec<Value> = cons.list_iter().map(|v| v.clone()).collect();
                debug!("Parsed Cons as list: {:?}", items);
                items
            }
            Value::Null => {
                debug!("Parsed Null value");
                vec![Value::Null]
            }
            _ => {
                warn!("Expected list format, got: {:?}", value);
                return Err(crate::error::ERPCError::InvalidMessageFormat(format!(
                    "Expected list, got: {:?}",
                    value
                )));
            }
        };

        debug!("Message items: {:?}", items);

        if items.len() < 2 {
            warn!(
                "Message too short: {} items, expected at least 2",
                items.len()
            );
            return Err(crate::error::ERPCError::InvalidMessageFormat(format!(
                "Message too short: {} items, expected at least 2",
                items.len()
            )));
        }

        let msg_type = match &items[0] {
            Value::Symbol(sym) => {
                let msg_type = sym.to_string();
                debug!("Message type: {}", msg_type);
                msg_type
            }
            _ => {
                warn!("Invalid message type: {:?}", items[0]);
                return Err(crate::error::ERPCError::InvalidMessageFormat(format!(
                    "Expected symbol for message type, got: {:?}",
                    items[0]
                )));
            }
        };

        let uid = match &items[1] {
            Value::Number(num) => {
                let uid = num.as_u64().ok_or_else(|| {
                    crate::error::ERPCError::InvalidMessageFormat(format!(
                        "Invalid UID value: {:?}",
                        num
                    ))
                })?;
                debug!("Message UID: {}", uid);
                uid
            }
            _ => {
                warn!("Invalid UID: {:?}", items[1]);
                return Err(crate::error::ERPCError::InvalidMessageFormat(format!(
                    "Expected number for UID, got: {:?}",
                    items[1]
                )));
            }
        };

        debug!("Processing message type: {} with UID: {}", msg_type, uid);

        match msg_type.as_str() {
            "call" => {
                if items.len() != 4 {
                    warn!("CALL message has {} elements, expected 4", items.len());
                    return Err(crate::error::ERPCError::InvalidMessageFormat(format!(
                        "Call message should have 4 elements, got {}",
                        items.len()
                    )));
                }
                let method = match &items[2] {
                    Value::Symbol(sym) => sym.to_string(),
                    Value::String(s) => s.to_string(),
                    _ => {
                        warn!("Invalid method name: {:?}", items[2]);
                        return Err(crate::error::ERPCError::InvalidMessageFormat(format!(
                            "Expected symbol/string for method name, got: {:?}",
                            items[2]
                        )));
                    }
                };
                debug!("Method call: {} with args: {:?}", method, items[3]);
                Ok(Message::new_call(uid, method, items[3].clone()))
            }
            "return" => {
                if items.len() != 3 {
                    warn!("RETURN message has {} elements, expected 3", items.len());
                    return Err(crate::error::ERPCError::InvalidMessageFormat(format!(
                        "Return message should have 3 elements, got {}",
                        items.len()
                    )));
                }
                debug!("Return message with result: {:?}", items[2]);
                Ok(Message::new_return(uid, items[2].clone()))
            }
            "return-error" => {
                if items.len() != 3 {
                    warn!(
                        "RETURN-ERROR message has {} elements, expected 3",
                        items.len()
                    );
                    return Err(crate::error::ERPCError::InvalidMessageFormat(format!(
                        "Return-error message should have 3 elements, got {}",
                        items.len()
                    )));
                }
                let error = match &items[2] {
                    Value::String(s) => s.to_string(),
                    _ => {
                        warn!("Invalid error message: {:?}", items[2]);
                        return Err(crate::error::ERPCError::InvalidMessageFormat(format!(
                            "Expected string for error message, got: {:?}",
                            items[2]
                        )));
                    }
                };
                debug!("Return error message: {}", error);
                Ok(Message::new_return_error(uid, error))
            }
            "epc-error" => {
                if items.len() != 3 {
                    warn!("EPC-ERROR message has {} elements, expected 3", items.len());
                    return Err(crate::error::ERPCError::InvalidMessageFormat(format!(
                        "EPC-error message should have 3 elements, got {}",
                        items.len()
                    )));
                }
                let error = match &items[2] {
                    Value::String(s) => s.to_string(),
                    _ => {
                        warn!("Invalid EPC error message: {:?}", items[2]);
                        return Err(crate::error::ERPCError::InvalidMessageFormat(format!(
                            "Expected string for EPC error, got: {:?}",
                            items[2]
                        )));
                    }
                };
                debug!("EPC error message: {}", error);
                Ok(Message::new_epc_error(uid, error))
            }
            "methods" => {
                if items.len() != 2 {
                    warn!("METHODS message has {} elements, expected 2", items.len());
                    return Err(crate::error::ERPCError::InvalidMessageFormat(format!(
                        "Methods message should have 2 elements, got {}",
                        items.len()
                    )));
                }
                debug!("Methods query message");
                Ok(Message::new_methods(uid))
            }
            _ => {
                warn!("Unknown message type: {}", msg_type);
                Err(crate::error::ERPCError::InvalidMessageFormat(format!(
                    "Unknown message type: {}",
                    msg_type
                )))
            }
        }
    }
}

/// Message framing utilities
pub struct Framer;

impl Framer {
    /// Frame a message with 6-byte length prefix
    pub fn frame(message: &[u8]) -> Bytes {
        let len = message.len();
        debug!("Framing message: {} bytes", len);

        let mut buf = BytesMut::with_capacity(6 + len);
        let len_str = format!("{:06x}", len);
        debug!("Length prefix: {}", len_str);

        buf.put_slice(len_str.as_bytes());
        buf.put_slice(message);

        let result = buf.freeze();
        debug!("Framed message total size: {} bytes", result.len());
        result
    }

    /// Parse length prefix from buffer
    pub fn parse_length(buf: &[u8]) -> Option<usize> {
        debug!("Parsing length from buffer: {} bytes", buf.len());

        if buf.len() < 6 {
            debug!("Buffer too short for length prefix: {} < 6", buf.len());
            return None;
        }

        let len_str = std::str::from_utf8(&buf[..6]).ok()?;
        debug!("Length string: {}", len_str);

        let result = usize::from_str_radix(len_str, 16).ok();
        debug!("Parsed length: {:?}", result);
        result
    }

    /// Extract complete message from buffer
    pub fn extract_message(buf: &mut BytesMut) -> Option<Bytes> {
        debug!("Extracting message from buffer: {} bytes", buf.len());

        if buf.len() < 6 {
            debug!("Buffer too short for header: {} < 6", buf.len());
            return None;
        }

        let len = Self::parse_length(buf)?;
        debug!("Message length: {}", len);

        let total_len = 6 + len;
        debug!("Total frame length: {}", total_len);

        if buf.len() < total_len {
            debug!(
                "Buffer too short for complete message: {} < {}",
                buf.len(),
                total_len
            );
            return None;
        }

        let message = buf[6..total_len].to_vec();
        debug!("Extracted message: {} bytes", message.len());

        buf.advance(total_len);
        debug!("Buffer advanced, remaining: {} bytes", buf.len());

        Some(Bytes::from(message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::new_call(123, "test", Value::string("hello"));
        assert!(matches!(msg, Message::Call { uid: 123, .. }));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let msg = Message::new_call(123, "test", Value::string("hello"));
        let sexp = msg.to_sexp().unwrap();
        let parsed = Message::from_sexp(&sexp).unwrap();

        match parsed {
            Message::Call { uid, method, args } => {
                assert_eq!(uid, 123);
                assert_eq!(method, "test");
                assert_eq!(args, Value::string("hello"));
            }
            _ => panic!("Expected Call message"),
        }
    }

    #[test]
    fn test_return_message() {
        let msg = Message::new_return(456, Value::from(42));
        let sexp = msg.to_sexp().unwrap();
        assert!(sexp.contains("return"));
        assert!(sexp.contains("42"));
    }

    #[test]
    fn test_framing() {
        let message = b"(call 123 test)";
        let framed = Framer::frame(message);
        assert_eq!(framed.len(), 21);
        assert_eq!(&framed[..6], b"00000f");
        assert_eq!(&framed[6..], message);
    }

    #[test]
    fn test_framing_roundtrip() {
        let message = b"(return 456 result)";
        let framed = Framer::frame(message);

        let mut buf = BytesMut::from(&framed[..]);
        let extracted = Framer::extract_message(&mut buf).unwrap();

        assert_eq!(extracted, Bytes::from_static(message));
    }
}
