use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use lexpr::from_str;
use crate::types::EpcValue;
use crate::error::{EpcError, EpcResult};

pub type SessionId = String;

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MessageType {
    Call,
    Return,
    ReturnError,
    EpcError,
    Methods,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub msg_type: MessageType,
    pub session_id: SessionId,
    pub payload: EpcValue,
}

impl Message {
    pub fn new_call(method_name: String, args: Vec<EpcValue>) -> Self {
        let session_id = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst).to_string();
        Message {
            msg_type: MessageType::Call,
            session_id,
            payload: EpcValue::List(vec![
                EpcValue::String(method_name),
                EpcValue::List(args),
            ]),
        }
    }
    
    pub fn new_return(session_id: SessionId, value: EpcValue) -> Self {
        Message {
            msg_type: MessageType::Return,
            session_id,
            payload: value,
        }
    }
    
    pub fn new_return_error(session_id: SessionId, error: String) -> Self {
        Message {
            msg_type: MessageType::ReturnError,
            session_id,
            payload: EpcValue::String(error),
        }
    }
    
    pub fn new_epc_error(session_id: SessionId, error: String) -> Self {
        Message {
            msg_type: MessageType::EpcError,
            session_id,
            payload: EpcValue::String(error),
        }
    }
    
    pub fn new_methods_query() -> Self {
        let session_id = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst).to_string();
        Message {
            msg_type: MessageType::Methods,
            session_id,
            payload: EpcValue::Nil,
        }
    }
    
    /// Serialize message to S-expression format for EPC protocol
    pub fn to_sexpr(&self) -> EpcResult<String> {
        // Custom serialization to ensure proper symbol format (not quoted strings)
        let msg_type_symbol = match self.msg_type {
            MessageType::Call => "call",
            MessageType::Return => "return", 
            MessageType::ReturnError => "return-error",
            MessageType::EpcError => "epc-error",
            MessageType::Methods => "methods",
        };
        
        let session_id_int = self.session_id.parse().unwrap_or(0);
        
        let result = if self.msg_type == MessageType::Call {
            // For call: (call session-id method-name arg1 arg2 ...)
            if let EpcValue::List(ref payload_list) = self.payload {
                if payload_list.len() >= 2 {
                    if let (EpcValue::String(ref method_name), EpcValue::List(ref args)) = 
                        (&payload_list[0], &payload_list[1]) {
                        // Build: (call session-id method-name arg1 arg2 ...)
                        let mut parts = vec![
                            msg_type_symbol.to_string(),
                            session_id_int.to_string(),
                            method_name.clone(),
                        ];
                        for arg in args {
                            parts.push(Self::serialize_value(arg)?);
                        }
                        format!("({})", parts.join(" "))
                    } else {
                        return Err(EpcError::serialization("Invalid call payload format".to_string()));
                    }
                } else {
                    return Err(EpcError::serialization("Call payload must have method and args".to_string()));
                }
            } else {
                return Err(EpcError::serialization("Call payload must be a list".to_string()));
            }
        } else {
            // For return/error: (msg-type session-id result)
            format!("({} {} {})", 
                msg_type_symbol, 
                session_id_int, 
                Self::serialize_value(&self.payload)?)
        };
        
        Ok(result)
    }
    
    fn serialize_value(value: &EpcValue) -> EpcResult<String> {
        match value {
            EpcValue::Nil => Ok("nil".to_string()),
            EpcValue::Bool(true) => Ok("t".to_string()),
            EpcValue::Bool(false) => Ok("nil".to_string()),
            EpcValue::Int(i) => Ok(i.to_string()),
            EpcValue::Float(f) => Ok(f.to_string()),
            EpcValue::String(s) => Ok(format!("\"{}\"", s.replace("\"", "\\\""))),
            EpcValue::Symbol(s) => Ok(s.clone()), // Symbols are not quoted
            EpcValue::List(list) => {
                let items: Result<Vec<String>, _> = list.iter()
                    .map(Self::serialize_value)
                    .collect();
                Ok(format!("({})", items?.join(" ")))
            },
            EpcValue::Dict(_) => {
                // For now, just serialize as nil - could implement proper dict serialization later
                Ok("nil".to_string())
            }
        }
    }
    
    /// Deserialize message from S-expression format
    pub fn from_sexpr(data: &str) -> EpcResult<Self> {
        let lexpr_value: lexpr::Value = from_str(data)
            .map_err(|e| EpcError::serialization(format!("Failed to parse S-expression: {}", e)))?;

        if let Some(list) = lexpr_value.to_vec() {
            if list.len() < 3 {
                return Err(EpcError::serialization("Message must have at least 3 elements".to_string()));
            }

            let msg_type = match list[0].as_symbol() {
                Some("call") => MessageType::Call,
                Some("return") => MessageType::Return,
                Some("return-error") => MessageType::ReturnError,
                Some("epc-error") => MessageType::EpcError,
                Some("methods") => MessageType::Methods,
                _ => return Err(EpcError::serialization("First element must be a message type symbol".to_string())),
            };

            let session_id = match &list[1] {
                lexpr::Value::String(s) => s.to_string(),
                lexpr::Value::Number(n) => n.to_string(),
                _ => return Err(EpcError::serialization("Second element must be a string or integer (session ID)".to_string())),
            };

            let payload = if msg_type == MessageType::Call {
                let method_name = match list[2].as_symbol() {
                    Some(s) => s.to_string(),
                    None => return Err(EpcError::serialization("Call message must have method name as 3rd element".to_string())),
                };

                let args: Vec<EpcValue> = list.into_iter().skip(3).map(EpcValue::from).collect();
                EpcValue::List(vec![
                    EpcValue::String(method_name),
                    EpcValue::List(args),
                ])
            } else {
                EpcValue::from(list[2].clone())
            };

            Ok(Message {
                msg_type,
                session_id,
                payload,
            })
        } else {
            Err(EpcError::serialization("Message must be a list".to_string()))
        }
    }
    
    /// Get method name from call message
    pub fn get_method_name(&self) -> Option<&str> {
        if self.msg_type == MessageType::Call {
            if let EpcValue::List(ref list) = self.payload {
                if let Some(EpcValue::String(ref method)) = list.get(0) {
                    return Some(method);
                }
            }
        }
        None
    }
    
    /// Get arguments from call message
    pub fn get_args(&self) -> Option<&Vec<EpcValue>> {
        if self.msg_type == MessageType::Call {
            if let EpcValue::List(ref list) = self.payload {
                if let Some(EpcValue::List(ref args)) = list.get(1) {
                    return Some(args);
                }
            }
        }
        None
    }
}