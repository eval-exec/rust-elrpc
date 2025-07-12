use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
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
        // Custom parser to handle symbols that serde-lexpr can't handle properly
        let list = Self::parse_sexpr_list(data)?;
        
        if list.len() < 3 {
            return Err(EpcError::serialization("Message must have at least 3 elements".to_string()));
        }
        
        // Extract message type
        let msg_type = match &list[0] {
            EpcValue::Symbol(s) => match s.as_str() {
                "call" => MessageType::Call,
                "return" => MessageType::Return,
                "return-error" => MessageType::ReturnError,
                "epc-error" => MessageType::EpcError,
                "methods" => MessageType::Methods,
                _ => return Err(EpcError::serialization(format!("Unknown message type: {}", s))),
            },
            _ => return Err(EpcError::serialization("First element must be a symbol".to_string())),
        };
        
        // Extract session ID - handle both integer and string formats
        let session_id = match &list[1] {
            EpcValue::String(s) => s.clone(),
            EpcValue::Int(i) => i.to_string(),
            _ => return Err(EpcError::serialization("Second element must be a string or integer (session ID)".to_string())),
        };
        
        // For call messages, the format is (call uid method-name args...)
        // For return messages, the format is (return uid result)
        let payload = if msg_type == MessageType::Call {
            // Extract method name (3rd element)
            let method_name = match list.get(2) {
                Some(EpcValue::Symbol(s)) => s.clone(),
                Some(EpcValue::String(s)) => s.clone(),
                _ => return Err(EpcError::serialization("Call message must have method name as 3rd element".to_string())),
            };
            
            // Extract args (4th element onwards) - flatten any nested lists from parsing
            let mut args: Vec<EpcValue> = list.into_iter().skip(3).collect();
            
            // If there's only one argument and it's a list, use its contents as the args
            // This handles the case where Emacs sends (call uid method (arg1 arg2))
            if args.len() == 1 {
                if let EpcValue::List(inner_args) = &args[0] {
                    args = inner_args.clone();
                }
            }
            
            // Create the expected payload format: [method_name, [args]]
            EpcValue::List(vec![
                EpcValue::String(method_name),
                EpcValue::List(args),
            ])
        } else {
            // For non-call messages, use the 3rd element as payload
            list.into_iter().nth(2).unwrap_or(EpcValue::Nil)
        };
        
        Ok(Message {
            msg_type,
            session_id,
            payload,
        })
    }
    
    /// Simple S-expression parser to handle symbols properly
    fn parse_sexpr_list(data: &str) -> EpcResult<Vec<EpcValue>> {
        let trimmed = data.trim();
        if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
            return Err(EpcError::serialization(format!("S-expression must be wrapped in parentheses - Raw: {}", data)));
        }

        let inner = &trimmed[1..trimmed.len()-1];
        let mut result = Vec::new();
        let mut chars = inner.chars().peekable();
        
        while let Some(char) = chars.peek() {
            match char {
                ' ' | '\t' | '\n' => {
                    // Skip whitespace
                    chars.next(); 
                }
                '(' => {
                    // Nested list
                    let list_str = Self::read_nested_list(&mut chars)?;
                    result.push(EpcValue::List(Self::parse_sexpr_list(&list_str)?));
                }
                '"' => {
                    // String literal
                    let string_literal = Self::read_string_literal(&mut chars)?;
                    result.push(EpcValue::String(string_literal));
                }
                _ => {
                    // Atom (symbol or number)
                    let token = Self::read_token(&mut chars);
                    if !token.is_empty() {
                        result.push(Self::parse_token(&token)?);
                    }
                }
            }
        }
        
        Ok(result)
    }

    fn read_nested_list(chars: &mut std::iter::Peekable<std::str::Chars>) -> EpcResult<String> {
        let mut list_str = String::new();
        let mut level = 0;
        
        while let Some(char) = chars.next() {
            list_str.push(char);
            match char {
                '(' => level += 1,
                ')' => {
                    level -= 1;
                    if level == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        
        if level == 0 {
            Ok(list_str)
        } else {
            Err(EpcError::serialization("Mismatched parentheses in S-expression".to_string()))
        }
    }

    fn read_string_literal(chars: &mut std::iter::Peekable<std::str::Chars>) -> EpcResult<String> {
        let mut s = String::new();
        chars.next(); // Consume opening quote
        
        let mut escaped = false;
        while let Some(char) = chars.next() {
            if escaped {
                s.push(char);
                escaped = false;
            } else if char == '\\' {
                escaped = true;
            } else if char == '"' {
                return Ok(s); // End of string
            } else {
                s.push(char);
            }
        }
        
        Err(EpcError::serialization("Unterminated string literal".to_string()))
    }

    fn read_token(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
        let mut token = String::new();
        while let Some(&char) = chars.peek() {
            if char.is_whitespace() || char == '(' || char == ')' {
                break;
            }
            token.push(chars.next().unwrap());
        }
        token
    }
    
    fn parse_token(token: &str) -> EpcResult<EpcValue> {
        if token == "nil" {
            Ok(EpcValue::Nil)
        } else if token == "t" {
            Ok(EpcValue::Bool(true))
        } else if let Ok(int_val) = token.parse::<i64>() {
            Ok(EpcValue::Int(int_val))
        } else if let Ok(float_val) = token.parse::<f64>() {
            Ok(EpcValue::Float(float_val))
        } else {
            // Everything else is a symbol
            Ok(EpcValue::Symbol(token.to_string()))
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