use bytes::{Buf, BufMut, Bytes, BytesMut};
use lexpr::Value;

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
    Return {
        uid: u64,
        result: Value,
    },
    
    /// Return an application error: (return-error uid error-message)
    ReturnError {
        uid: u64,
        error: String,
    },
    
    /// Return a protocol error: (epc-error uid error-message)
    EPCError {
        uid: u64,
        error: String,
    },
    
    /// Query available methods: (methods uid)
    Methods {
        uid: u64,
    },
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
        let sexp = match self {
            Message::Call { uid, method, args } => {
                Value::list(vec![
                    Value::symbol("call"),
                    Value::from(*uid as i64),
                    Value::symbol(method.clone()),
                    args.clone(),
                ])
            }
            Message::Return { uid, result } => {
                Value::list(vec![
                    Value::symbol("return"),
                    Value::from(*uid as i64),
                    result.clone(),
                ])
            }
            Message::ReturnError { uid, error } => {
                Value::list(vec![
                    Value::symbol("return-error"),
                    Value::from(*uid as i64),
                    Value::string(error.clone()),
                ])
            }
            Message::EPCError { uid, error } => {
                Value::list(vec![
                    Value::symbol("epc-error"),
                    Value::from(*uid as i64),
                    Value::string(error.clone()),
                ])
            }
            Message::Methods { uid } => {
                Value::list(vec![
                    Value::symbol("methods"),
                    Value::from(*uid as i64),
                ])
            }
        };
        
        lexpr::to_string(&sexp).map_err(|e| crate::error::ERPCError::SerializationError(e.to_string()))
    }

    /// Parse message from S-expression string
    pub fn from_sexp(s: &str) -> std::result::Result<Self, crate::error::ERPCError> {
        let value = lexpr::from_str(s)?;
        
        // Handle both Cons and proper list formats
        let items: Vec<Value> = match value {
            Value::Cons(cons) => {
                cons.list_iter().map(|v| v.clone()).collect()
            }
            Value::Null => vec![Value::Null],
            _ => return Err(crate::error::ERPCError::InvalidMessageFormat(
                "Expected list".to_string(),
            )),
        };
        
        if items.len() < 2 {
            return Err(crate::error::ERPCError::InvalidMessageFormat(
                "Message too short".to_string(),
            ));
        }
        
        let msg_type = match &items[0] {
            Value::Symbol(sym) => sym.to_string(),
            _ => return Err(crate::error::ERPCError::InvalidMessageFormat(
                "Expected symbol for message type".to_string(),
            )),
        };
        
        let uid = match &items[1] {
            Value::Number(num) => num.as_u64().ok_or_else(|| {
                crate::error::ERPCError::InvalidMessageFormat("Invalid UID".to_string())
            })?,
            _ => return Err(crate::error::ERPCError::InvalidMessageFormat(
                "Expected number for UID".to_string(),
            )),
        };
        
        match msg_type.as_str() {
            "call" => {
                if items.len() != 4 {
                    return Err(crate::error::ERPCError::InvalidMessageFormat(
                        "Call message should have 4 elements".to_string(),
                    ));
                }
                let method = match &items[2] {
                    Value::Symbol(sym) => sym.to_string(),
                    Value::String(s) => s.to_string(),
                    _ => return Err(crate::error::ERPCError::InvalidMessageFormat(
                        "Expected symbol/string for method name".to_string(),
                    )),
                };
                Ok(Message::new_call(uid, method, items[3].clone()))
            }
            "return" => {
                if items.len() != 3 {
                    return Err(crate::error::ERPCError::InvalidMessageFormat(
                        "Return message should have 3 elements".to_string(),
                    ));
                }
                Ok(Message::new_return(uid, items[2].clone()))
            }
            "return-error" => {
                if items.len() != 3 {
                    return Err(crate::error::ERPCError::InvalidMessageFormat(
                        "Return-error message should have 3 elements".to_string(),
                    ));
                }
                let error = match &items[2] {
                    Value::String(s) => s.to_string(),
                    _ => return Err(crate::error::ERPCError::InvalidMessageFormat(
                        "Expected string for error message".to_string(),
                    )),
                };
                Ok(Message::new_return_error(uid, error))
            }
            "epc-error" => {
                if items.len() != 3 {
                    return Err(crate::error::ERPCError::InvalidMessageFormat(
                        "EPC-error message should have 3 elements".to_string(),
                    ));
                }
                let error = match &items[2] {
                    Value::String(s) => s.to_string(),
                    _ => return Err(crate::error::ERPCError::InvalidMessageFormat(
                        "Expected string for EPC error".to_string(),
                    )),
                };
                Ok(Message::new_epc_error(uid, error))
            }
            "methods" => {
                if items.len() != 2 {
                    return Err(crate::error::ERPCError::InvalidMessageFormat(
                        "Methods message should have 2 elements".to_string(),
                    ));
                }
                Ok(Message::new_methods(uid))
            }
            _ => Err(crate::error::ERPCError::InvalidMessageFormat(
                format!("Unknown message type: {}", msg_type),
            )),
        }
    }
}

/// Message framing utilities
pub struct Framer;

impl Framer {
    /// Frame a message with 6-byte length prefix
    pub fn frame(message: &[u8]) -> Bytes {
        let len = message.len();
        let mut buf = BytesMut::with_capacity(6 + len);
        buf.put_slice(&format!("{:06x}", len).as_bytes());
        buf.put_slice(message);
        buf.freeze()
    }

    /// Parse length prefix from buffer
    pub fn parse_length(buf: &[u8]) -> Option<usize> {
        if buf.len() < 6 {
            return None;
        }
        
        let len_str = std::str::from_utf8(&buf[..6]).ok()?;
        usize::from_str_radix(len_str, 16).ok()
    }

    /// Extract complete message from buffer
    pub fn extract_message(buf: &mut BytesMut) -> Option<Bytes> {
        if buf.len() < 6 {
            return None;
        }
        
        let len = Self::parse_length(buf)?;
        let total_len = 6 + len;
        
        if buf.len() < total_len {
            return None;
        }
        
        let message = buf[6..total_len].to_vec();
        buf.advance(total_len);
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