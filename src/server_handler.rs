use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::Arc;
use std::collections::HashMap;
use log::{debug, error};

use crate::error::{EpcError, EpcResult};
use crate::protocol::MethodHandler;
use crate::message::{Message, MessageType};
use crate::types::EpcValue;

pub struct ServerConnectionHandler {
    stream: TcpStream,
    methods: Arc<std::sync::Mutex<HashMap<String, MethodHandler>>>,
}

impl ServerConnectionHandler {
    pub fn new(stream: TcpStream, methods: Arc<std::sync::Mutex<HashMap<String, MethodHandler>>>) -> Self {
        debug!("Creating new ServerConnectionHandler");
        Self { stream, methods }
    }
    
    pub async fn handle(mut self) -> EpcResult<()> {
        debug!("ServerConnectionHandler starting to handle connection");
        let mut buffer = vec![0u8; 4096];
        let mut message_buffer = Vec::new();
        let mut expected_length: Option<usize> = None;
        
        loop {
            let bytes_read = self.stream.read(&mut buffer).await?;
            
            if bytes_read == 0 {
                debug!("Client disconnected");
                break;
            }
            
            debug!("Server read {} bytes", bytes_read);
            message_buffer.extend_from_slice(&buffer[..bytes_read]);
            
            // Process complete messages
            loop {
                if expected_length.is_none() && message_buffer.len() >= 6 {
                    let header = String::from_utf8_lossy(&message_buffer[..6]);
                    expected_length = Some(
                        usize::from_str_radix(&header, 16)
                            .map_err(|_| EpcError::InvalidMessage)?
                    );
                    message_buffer.drain(..6);
                }
                
                if let Some(length) = expected_length {
                    if message_buffer.len() >= length {
                        let message_data = message_buffer.drain(..length).collect::<Vec<u8>>();
                        let message_str = String::from_utf8(message_data)
                            .map_err(|_| EpcError::InvalidMessage)?;
                        
                        debug!("Server parsing message: {}", message_str);
                        
                        match Message::from_sexpr(&message_str) {
                            Ok(message) => {
                                self.handle_message(message).await?;
                            }
                            Err(e) => {
                                error!("Failed to parse message: {} - Raw: {}", e, message_str);
                            }
                        }
                        
                        expected_length = None;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    async fn handle_message(&mut self, message: Message) -> EpcResult<()> {
        match message.msg_type {
            MessageType::Call => {
                let method_name = message.get_method_name()
                    .ok_or_else(|| EpcError::protocol("Invalid call message"))?;
                
                let empty_args = vec![];
                let args = message.get_args().unwrap_or(&empty_args);
                
                let response = {
                    let result = {
                        let methods = self.methods.lock().unwrap();
                        match methods.get(method_name) {
                            Some(handler) => handler(args),
                            None => Err(EpcError::MethodNotFound(method_name.to_string())),
                        }
                    };
                    
                    match result {
                        Ok(value) => {
                            debug!("Server method call successful, returning: {:?}", value);
                            Message::new_return(message.session_id, value)
                        },
                        Err(e) => {
                            debug!("Server method call error: {}", e);
                            Message::new_return_error(message.session_id, e.to_string())
                        },
                    }
                };
                
                self.send_message(response).await?;
            }
            MessageType::Methods => {
                let method_list: Vec<EpcValue> = {
                    let methods = self.methods.lock().unwrap();
                    methods.keys()
                        .map(|name| EpcValue::List(vec![
                            EpcValue::Symbol(name.clone()),
                            EpcValue::String("".to_string()),
                            EpcValue::String("".to_string()),
                        ]))
                        .collect()
                };
                
                let response = Message::new_return(
                    message.session_id,
                    EpcValue::List(method_list),
                );
                
                self.send_message(response).await?;
            }
            _ => {
                debug!("Server ignoring message type: {:?}", message.msg_type);
            }
        }
        
        Ok(())
    }
    
    async fn send_message(&mut self, message: Message) -> EpcResult<()> {
        let serialized = message.to_sexpr()?;
        let mut full_message = serialized;
        full_message.push('\n');
        let length = full_message.len();
        let header = format!("{:06x}", length);
        
        debug!("Server sending: {}{}", header, full_message.trim());
        
        self.stream.write_all(header.as_bytes()).await?;
        self.stream.write_all(full_message.as_bytes()).await?;
        self.stream.flush().await?;
        
        debug!("Server sent {} bytes", length);
        Ok(())
    }
}