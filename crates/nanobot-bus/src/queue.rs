//! Message bus with async queues

use crate::events::{InboundMessage, OutboundMessage};
use tokio::sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error};

/// Message bus for inter-component communication
#[derive(Clone)]
pub struct MessageBus {
    inbound_tx: mpsc::Sender<InboundMessage>,
    inbound_rx: Arc<Mutex<mpsc::Receiver<InboundMessage>>>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    outbound_rx: Arc<Mutex<mpsc::Receiver<OutboundMessage>>>,
}

impl MessageBus {
    /// Create a new message bus with default buffer sizes
    pub fn new() -> Self {
        let (inbound_tx, inbound_rx) = mpsc::channel(100);
        let (outbound_tx, outbound_rx) = mpsc::channel(100);

        Self {
            inbound_tx,
            inbound_rx: Arc::new(Mutex::new(inbound_rx)),
            outbound_tx,
            outbound_rx: Arc::new(Mutex::new(outbound_rx)),
        }
    }

    /// Create a new message bus with custom buffer sizes
    pub fn with_buffer_size(inbound_size: usize, outbound_size: usize) -> Self {
        let (inbound_tx, inbound_rx) = mpsc::channel(inbound_size);
        let (outbound_tx, outbound_rx) = mpsc::channel(outbound_size);

        Self {
            inbound_tx,
            inbound_rx: Arc::new(Mutex::new(inbound_rx)),
            outbound_tx,
            outbound_rx: Arc::new(Mutex::new(outbound_rx)),
        }
    }

    /// Publish an inbound message
    pub async fn publish_inbound(&self, msg: InboundMessage) -> Result<(), BusError> {
        debug!("Publishing inbound message: {} -> {}", msg.channel, msg.chat_id);
        self.inbound_tx
            .send(msg)
            .await
            .map_err(|_| BusError::ChannelClosed)
    }

    /// Publish an outbound message
    pub async fn publish_outbound(&self, msg: OutboundMessage) -> Result<(), BusError> {
        debug!("Publishing outbound message: {} -> {}", msg.channel, msg.chat_id);
        self.outbound_tx
            .send(msg)
            .await
            .map_err(|_| BusError::ChannelClosed)
    }

    /// Consume an inbound message (blocking)
    pub async fn consume_inbound(&self) -> Result<InboundMessage, BusError> {
        let mut rx = self.inbound_rx.lock().await;
        rx.recv().await.ok_or(BusError::ChannelClosed)
    }

    /// Consume an outbound message (blocking)
    pub async fn consume_outbound(&self) -> Result<OutboundMessage, BusError> {
        let mut rx = self.outbound_rx.lock().await;
        rx.recv().await.ok_or(BusError::ChannelClosed)
    }

    /// Try to consume an inbound message (non-blocking)
    pub async fn try_consume_inbound(&self) -> Option<InboundMessage> {
        let mut rx = self.inbound_rx.lock().await;
        rx.try_recv().ok()
    }

    /// Try to consume an outbound message (non-blocking)
    pub async fn try_consume_outbound(&self) -> Option<OutboundMessage> {
        let mut rx = self.outbound_rx.lock().await;
        rx.try_recv().ok()
    }

    /// Get the inbound sender
    pub fn inbound_sender(&self) -> mpsc::Sender<InboundMessage> {
        self.inbound_tx.clone()
    }

    /// Get the outbound sender
    pub fn outbound_sender(&self) -> mpsc::Sender<OutboundMessage> {
        self.outbound_tx.clone()
    }

    /// Split the bus into separate inbound/outbound handles
    pub fn split(self) -> (InboundHandle, OutboundHandle) {
        (
            InboundHandle {
                tx: self.inbound_tx,
                rx: self.inbound_rx,
            },
            OutboundHandle {
                tx: self.outbound_tx,
                rx: self.outbound_rx,
            },
        )
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Inbound message handle
pub struct InboundHandle {
    tx: mpsc::Sender<InboundMessage>,
    rx: Arc<Mutex<mpsc::Receiver<InboundMessage>>>,
}

impl InboundHandle {
    pub async fn publish(&self, msg: InboundMessage) -> Result<(), BusError> {
        self.tx
            .send(msg)
            .await
            .map_err(|_| BusError::ChannelClosed)
    }

    pub async fn consume(&self) -> Result<InboundMessage, BusError> {
        let mut rx = self.rx.lock().await;
        rx.recv().await.ok_or(BusError::ChannelClosed)
    }
}

/// Outbound message handle
pub struct OutboundHandle {
    tx: mpsc::Sender<OutboundMessage>,
    rx: Arc<Mutex<mpsc::Receiver<OutboundMessage>>>,
}

impl OutboundHandle {
    pub async fn publish(&self, msg: OutboundMessage) -> Result<(), BusError> {
        self.tx
            .send(msg)
            .await
            .map_err(|_| BusError::ChannelClosed)
    }

    pub async fn consume(&self) -> Result<OutboundMessage, BusError> {
        let mut rx = self.rx.lock().await;
        rx.recv().await.ok_or(BusError::ChannelClosed)
    }
}

/// Bus error types
#[derive(Debug, thiserror::Error)]
pub enum BusError {
    #[error("Channel closed")]
    ChannelClosed,

    #[error("Bus error: {0}")]
    Other(String),
}
