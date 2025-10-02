use axum::extract::ws::{Message, WebSocket};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

use crate::errors::{LogLineError, Result};

/// Envelope used to encode/decode WebSocket messages between services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketEnvelope {
    pub event: String,
    pub payload: serde_json::Value,
}

impl WebSocketEnvelope {
    pub fn new(event: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            event: event.into(),
            payload,
        }
    }

    pub fn to_message(&self) -> Result<Message> {
        let serialized = serde_json::to_string(self)
            .map_err(|err| LogLineError::SerializationError(err.to_string()))?;
        Ok(Message::Text(serialized))
    }

    pub fn from_message(message: Message) -> Result<Self> {
        match message {
            Message::Text(text) => {
                let envelope: WebSocketEnvelope = serde_json::from_str(&text)?;
                Ok(envelope)
            }
            Message::Binary(bytes) => {
                let envelope: WebSocketEnvelope = serde_json::from_slice(&bytes)?;
                Ok(envelope)
            }
            Message::Close(frame) => Err(LogLineError::GeneralError(format!(
                "Conexão encerrada: {:?}",
                frame
            ))),
            Message::Ping(_) | Message::Pong(_) => Err(LogLineError::GeneralError(
                "Mensagens de controle não são suportadas pela camada de protocolo".into(),
            )),
        }
    }
}

/// Helper struct that splits a WebSocket into sender/receiver halves with protocol helpers.
pub struct WebSocketChannel {
    sender: SplitSink<WebSocket, Message>,
    receiver: SplitStream<WebSocket>,
}

impl WebSocketChannel {
    pub fn new(socket: WebSocket) -> Self {
        let (sender, receiver) = socket.split();
        Self { sender, receiver }
    }

    pub async fn send(&mut self, envelope: &WebSocketEnvelope) -> Result<()> {
        self.sender.send(envelope.to_message()?).await?;
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<Option<WebSocketEnvelope>> {
        if let Some(message) = self.receiver.next().await {
            let msg = message?;
            Ok(Some(WebSocketEnvelope::from_message(msg)?))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trip() {
        let envelope = WebSocketEnvelope::new("test", serde_json::json!({"value": 42}));
        let message = envelope.to_message().expect("serialize");
        let decoded = WebSocketEnvelope::from_message(message).expect("decode");
        assert_eq!(decoded.event, "test");
    }
}
