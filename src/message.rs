use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::types::EpcValue;
use crate::error::{EpcError, EpcResult};

pub type SessionId = String;

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
        let session_id = Uuid::new_v4().to_string();
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
        let session_id = Uuid::new_v4().to_string();
        Message {
            msg_type: MessageType::Methods,
            session_id,
            payload: EpcValue::Nil,
        }
    }
    
    /// Serialize message to S-expression format for EPC protocol
    pub fn to_sexpr(&self) -> EpcResult<String> {
        let msg_tuple = (
            &self.msg_type,
            &self.session_id,
            &self.payload,
        );
        
        serde_lexpr::to_string(&msg_tuple)
            .map_err(|e| EpcError::serialization(format!("Failed to serialize message: {}", e)))
    }
    
    /// Deserialize message from S-expression format
    pub fn from_sexpr(data: &str) -> EpcResult<Self> {
        let (msg_type, session_id, payload): (MessageType, SessionId, EpcValue) = 
            serde_lexpr::from_str(data)
                .map_err(|e| EpcError::serialization(format!("Failed to deserialize message: {}", e)))?;
        
        Ok(Message {
            msg_type,
            session_id,
            payload,
        })
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