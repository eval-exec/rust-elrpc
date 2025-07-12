use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, oneshot};
use log::{debug, error, warn};

use crate::error::{EpcError, EpcResult};
use crate::message::{Message, MessageType, SessionId};
use crate::types::EpcValue;

pub type MethodHandler = Box<dyn Fn(&[EpcValue]) -> EpcResult<EpcValue> + Send + Sync>;
pub type PendingCall = oneshot::Sender<EpcResult<EpcValue>>;

pub struct EpcConnection {
    stream: Arc<Mutex<TcpStream>>,
    methods: Arc<Mutex<HashMap<String, MethodHandler>>>,
    pending_calls: Arc<Mutex<HashMap<SessionId, PendingCall>>>,
    message_sender: mpsc::UnboundedSender<Message>,
    shutdown_sender: Option<oneshot::Sender<()>>,
}

impl EpcConnection {
    pub async fn new(stream: TcpStream) -> EpcResult<Self> {
        let stream = Arc::new(Mutex::new(stream));
        let methods = Arc::new(Mutex::new(HashMap::new()));
        let pending_calls = Arc::new(Mutex::new(HashMap::new()));
        
        let (message_sender, mut message_receiver) = mpsc::unbounded_channel::<Message>();
        let (shutdown_sender, shutdown_receiver) = oneshot::channel::<()>();
        
        // Clone for the writer task
        let stream_writer = Arc::clone(&stream);
        let _message_sender_clone = message_sender.clone();
        
        // Spawn writer task
        tokio::spawn(async move {
            let mut shutdown_receiver = shutdown_receiver;
            
            loop {
                tokio::select! {
                    msg = message_receiver.recv() => {
                        match msg {
                            Some(message) => {
                                if let Err(e) = Self::send_message(&stream_writer, &message).await {
                                    error!("Failed to send message: {}", e);
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    _ = &mut shutdown_receiver => {
                        debug!("Writer task shutting down");
                        break;
                    }
                }
            }
        });
        
        // Clone for the reader task
        let stream_reader = Arc::clone(&stream);
        let methods_reader = Arc::clone(&methods);
        let pending_calls_reader = Arc::clone(&pending_calls);
        let message_sender_reader = message_sender.clone();
        
        // Spawn reader task
        tokio::spawn(async move {
            if let Err(e) = Self::read_loop(
                stream_reader,
                methods_reader,
                pending_calls_reader,
                message_sender_reader,
            ).await {
                error!("Reader task error: {}", e);
            }
        });
        
        Ok(EpcConnection {
            stream,
            methods,
            pending_calls,
            message_sender,
            shutdown_sender: Some(shutdown_sender),
        })
    }
    
    async fn send_message(stream: &Arc<Mutex<TcpStream>>, message: &Message) -> EpcResult<()> {
        let serialized = message.to_sexpr()?;
        let length = serialized.len();
        let header = format!("{:06x}", length);
        
        let mut stream = stream.lock().await;
        stream.write_all(header.as_bytes()).await?;
        stream.write_all(serialized.as_bytes()).await?;
        stream.flush().await?;
        
        debug!("Sent message: {} bytes", length);
        Ok(())
    }
    
    async fn read_loop(
        stream: Arc<Mutex<TcpStream>>,
        methods: Arc<Mutex<HashMap<String, MethodHandler>>>,
        pending_calls: Arc<Mutex<HashMap<SessionId, PendingCall>>>,
        message_sender: mpsc::UnboundedSender<Message>,
    ) -> EpcResult<()> {
        let mut buffer = vec![0u8; 4096];
        let mut message_buffer = Vec::new();
        let mut expected_length: Option<usize> = None;
        
        loop {
            let bytes_read = {
                let mut stream = stream.lock().await;
                stream.read(&mut buffer).await?
            };
            
            if bytes_read == 0 {
                return Err(EpcError::ConnectionClosed);
            }
            
            message_buffer.extend_from_slice(&buffer[..bytes_read]);
            
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
                        
                        match Message::from_sexpr(&message_str) {
                            Ok(message) => {
                                Self::handle_message(
                                    message,
                                    &methods,
                                    &pending_calls,
                                    &message_sender,
                                ).await;
                            }
                            Err(e) => {
                                warn!("Failed to parse message: {}", e);
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
    }
    
    async fn handle_message(
        message: Message,
        methods: &Arc<Mutex<HashMap<String, MethodHandler>>>,
        pending_calls: &Arc<Mutex<HashMap<SessionId, PendingCall>>>,
        message_sender: &mpsc::UnboundedSender<Message>,
    ) {
        match message.msg_type {
            MessageType::Call => {
                Self::handle_call(message, methods, message_sender).await;
            }
            MessageType::Return => {
                Self::handle_return(message, pending_calls).await;
            }
            MessageType::ReturnError | MessageType::EpcError => {
                Self::handle_error_return(message, pending_calls).await;
            }
            MessageType::Methods => {
                Self::handle_methods_query(message, methods, message_sender).await;
            }
        }
    }
    
    async fn handle_call(
        message: Message,
        methods: &Arc<Mutex<HashMap<String, MethodHandler>>>,
        message_sender: &mpsc::UnboundedSender<Message>,
    ) {
        let method_name = match message.get_method_name() {
            Some(name) => name,
            None => {
                let error_msg = Message::new_epc_error(
                    message.session_id,
                    "Invalid call message format".to_string(),
                );
                let _ = message_sender.send(error_msg);
                return;
            }
        };
        
        let empty_args = vec![];
        let args = message.get_args().unwrap_or(&empty_args);
        
        let result = {
            let methods = methods.lock().await;
            match methods.get(method_name) {
                Some(handler) => handler(args),
                None => Err(EpcError::MethodNotFound(method_name.to_string())),
            }
        };
        
        let response = match result {
            Ok(value) => Message::new_return(message.session_id, value),
            Err(EpcError::MethodNotFound(method)) => {
                Message::new_epc_error(message.session_id, format!("Method not found: {}", method))
            }
            Err(e) => Message::new_return_error(message.session_id, e.to_string()),
        };
        
        let _ = message_sender.send(response);
    }
    
    async fn handle_return(
        message: Message,
        pending_calls: &Arc<Mutex<HashMap<SessionId, PendingCall>>>,
    ) {
        let mut pending = pending_calls.lock().await;
        if let Some(sender) = pending.remove(&message.session_id) {
            let _ = sender.send(Ok(message.payload));
        }
    }
    
    async fn handle_error_return(
        message: Message,
        pending_calls: &Arc<Mutex<HashMap<SessionId, PendingCall>>>,
    ) {
        let mut pending = pending_calls.lock().await;
        if let Some(sender) = pending.remove(&message.session_id) {
            let error = match message.payload {
                EpcValue::String(msg) => match message.msg_type {
                    MessageType::ReturnError => EpcError::Application(msg),
                    MessageType::EpcError => EpcError::Protocol(msg),
                    _ => EpcError::Protocol("Unknown error".to_string()),
                },
                _ => EpcError::Protocol("Invalid error message format".to_string()),
            };
            let _ = sender.send(Err(error));
        }
    }
    
    async fn handle_methods_query(
        message: Message,
        methods: &Arc<Mutex<HashMap<String, MethodHandler>>>,
        message_sender: &mpsc::UnboundedSender<Message>,
    ) {
        let methods = methods.lock().await;
        let method_list: Vec<EpcValue> = methods.keys()
            .map(|name| EpcValue::List(vec![
                EpcValue::Symbol(name.clone()),
                EpcValue::Nil, // argdoc placeholder
                EpcValue::Nil, // docstring placeholder
            ]))
            .collect();
        
        let response = Message::new_return(
            message.session_id,
            EpcValue::List(method_list),
        );
        
        let _ = message_sender.send(response);
    }
    
    pub async fn register_method<F>(&self, name: String, handler: F)
    where
        F: Fn(&[EpcValue]) -> EpcResult<EpcValue> + Send + Sync + 'static,
    {
        let mut methods = self.methods.lock().await;
        methods.insert(name, Box::new(handler));
    }
    
    pub async fn call_method(&self, method_name: String, args: Vec<EpcValue>) -> EpcResult<EpcValue> {
        let message = Message::new_call(method_name, args);
        let session_id = message.session_id.clone();
        
        let (sender, receiver) = oneshot::channel();
        
        {
            let mut pending = self.pending_calls.lock().await;
            pending.insert(session_id, sender);
        }
        
        self.message_sender.send(message)
            .map_err(|_| EpcError::ConnectionClosed)?;
        
        receiver.await
            .map_err(|_| EpcError::ConnectionClosed)?
    }
    
    pub async fn query_methods(&self) -> EpcResult<Vec<EpcValue>> {
        let message = Message::new_methods_query();
        let session_id = message.session_id.clone();
        
        let (sender, receiver) = oneshot::channel();
        
        {
            let mut pending = self.pending_calls.lock().await;
            pending.insert(session_id, sender);
        }
        
        self.message_sender.send(message)
            .map_err(|_| EpcError::ConnectionClosed)?;
        
        let result = receiver.await
            .map_err(|_| EpcError::ConnectionClosed)??;
        
        match result {
            EpcValue::List(methods) => Ok(methods),
            _ => Err(EpcError::Protocol("Invalid methods response".to_string())),
        }
    }
}

impl Drop for EpcConnection {
    fn drop(&mut self) {
        if let Some(sender) = self.shutdown_sender.take() {
            let _ = sender.send(());
        }
    }
}