use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use lexpr::Value as LexprValue;

/// Represents values that can be serialized/deserialized in EPC protocol
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EpcValue {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Symbol(String),
    List(Vec<EpcValue>),
    Dict(HashMap<String, EpcValue>),
}

impl EpcValue {
    pub fn is_nil(&self) -> bool {
        matches!(self, EpcValue::Nil)
    }
    
    pub fn as_string(&self) -> Option<&str> {
        match self {
            EpcValue::String(s) => Some(s),
            _ => None,
        }
    }
    
    pub fn as_int(&self) -> Option<i64> {
        match self {
            EpcValue::Int(i) => Some(*i),
            _ => None,
        }
    }
    
    pub fn as_list(&self) -> Option<&Vec<EpcValue>> {
        match self {
            EpcValue::List(list) => Some(list),
            _ => None,
        }
    }
}

impl From<()> for EpcValue {
    fn from(_: ()) -> Self {
        EpcValue::Nil
    }
}

impl From<bool> for EpcValue {
    fn from(b: bool) -> Self {
        EpcValue::Bool(b)
    }
}

impl From<i32> for EpcValue {
    fn from(i: i32) -> Self {
        EpcValue::Int(i as i64)
    }
}

impl From<i64> for EpcValue {
    fn from(i: i64) -> Self {
        EpcValue::Int(i)
    }
}

impl From<f64> for EpcValue {
    fn from(f: f64) -> Self {
        EpcValue::Float(f)
    }
}

impl From<String> for EpcValue {
    fn from(s: String) -> Self {
        EpcValue::String(s)
    }
}

impl From<&str> for EpcValue {
    fn from(s: &str) -> Self {
        EpcValue::String(s.to_string())
    }
}

impl<T: Into<EpcValue>> From<Vec<T>> for EpcValue {
    fn from(vec: Vec<T>) -> Self {
        EpcValue::List(vec.into_iter().map(|x| x.into()).collect())
    }
}

impl<T: Into<EpcValue>> From<HashMap<String, T>> for EpcValue {
    fn from(map: HashMap<String, T>) -> Self {
        EpcValue::Dict(map.into_iter().map(|(k, v)| (k, v.into())).collect())
    }
}

impl From<LexprValue> for EpcValue {
    fn from(value: LexprValue) -> Self {
        match value {
            LexprValue::Nil => EpcValue::Nil,
            LexprValue::Bool(b) => EpcValue::Bool(b),
            LexprValue::Number(n) => {
                if n.is_i64() {
                    EpcValue::Int(n.as_i64().unwrap())
                } else {
                    EpcValue::Float(n.as_f64().unwrap())
                }
            }
            LexprValue::String(s) => EpcValue::String(s.to_string()),
            LexprValue::Symbol(s) => EpcValue::Symbol(s.to_string()),
            LexprValue::Cons(cons) => {
                let (car, cdr) = cons.into_pair();
                if cdr == LexprValue::Nil {
                    EpcValue::List(vec![EpcValue::from(car)])
                } else {
                     let mut vec = vec![EpcValue::from(car)];
                     let mut current = cdr;
                     while let LexprValue::Cons(cons) = current {
                         let (car, cdr) = cons.into_pair();
                         vec.push(EpcValue::from(car));
                         current = cdr;
                     }
                     EpcValue::List(vec)
                }
            }
            _ => EpcValue::Nil, // Fallback for other types
        }
    }
}