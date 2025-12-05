use corelink_core::{CoreLinkCodec, Message};
use futures::{AsyncRead, AsyncWrite, Future};
use libp2p_core::upgrade::{InboundUpgrade, OutboundUpgrade, UpgradeInfo};
use libp2p_swarm::{
    handler::ConnectionEvent, ConnectionHandler, ConnectionHandlerEvent, Stream, StreamProtocol,
    SubstreamProtocol,
};
use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tracing::{debug, error, info};

#[derive(Debug, Clone)]
pub struct CoreLinkProtocol;

impl UpgradeInfo for CoreLinkProtocol {
    type Info = StreamProtocol;
    type InfoIter = std::iter::Once<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        std::iter::once(StreamProtocol::new("/corelink/msg/1.0.0"))
    }
}

impl<T> InboundUpgrade<T> for CoreLinkProtocol
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = T;
    type Error = io::Error;
    type Future = futures::future::Ready<Result<Self::Output, Self::Error>>;

    fn upgrade_inbound(self, socket: T, _: Self::Info) -> Self::Future {
        info!("🔵 Inbound protocol upgrade");
        futures::future::ok(socket)
    }
}

impl<T> OutboundUpgrade<T> for CoreLinkProtocol
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = T;
    type Error = io::Error;
    type Future = futures::future::Ready<Result<Self::Output, Self::Error>>;

    fn upgrade_outbound(self, socket: T, _: Self::Info) -> Self::Future {
        info!("🔴 Outbound protocol upgrade");
        futures::future::ok(socket)
    }
}

#[derive(Debug)]
pub enum CoreLinkHandlerEvent {
    MessageReceived(Message),
    MessageSent,
    SendError(String),
}

type ReadFuture = Pin<Box<dyn Future<Output = Result<(Stream, Message), io::Error>> + Send>>;
type WriteFuture = Pin<Box<dyn Future<Output = Result<Stream, io::Error>> + Send>>;

enum StreamState {
    Idle,
    Reading(ReadFuture),
    Writing(WriteFuture),
}

pub struct CoreLinkHandler {
    inbound_stream: Option<Stream>,
    outbound_stream: Option<Stream>,
    inbound_state: StreamState,
    outbound_state: StreamState,
    pending_messages: VecDeque<Message>,
    events: VecDeque<CoreLinkHandlerEvent>,
    dial_upgrade_failures: u32,
    listen_upgrade_failures: u32,
    can_request_outbound: bool,
}

impl CoreLinkHandler {
    pub fn new() -> Self {
        debug!("Creating new CoreLinkHandler");
        Self {
            inbound_stream: None,
            outbound_stream: None,
            inbound_state: StreamState::Idle,
            outbound_state: StreamState::Idle,
            pending_messages: VecDeque::new(),
            events: VecDeque::new(),
            dial_upgrade_failures: 0,
            listen_upgrade_failures: 0,
            can_request_outbound: false,
        }
    }
}

impl ConnectionHandler for CoreLinkHandler {
    type FromBehaviour = Message;
    type ToBehaviour = CoreLinkHandlerEvent;
    type InboundProtocol = CoreLinkProtocol;
    type OutboundProtocol = CoreLinkProtocol;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        SubstreamProtocol::new(CoreLinkProtocol, ())
    }

    fn on_behaviour_event(&mut self, message: Self::FromBehaviour) {
        info!(
            "🟢 Handler received message from behaviour: {:?}",
            message.msg_type
        );
        self.pending_messages.push_back(message);
    }

    fn poll(
        &mut self,
        cx: &mut Context,
    ) -> Poll<
        ConnectionHandlerEvent<Self::OutboundProtocol, Self::OutboundOpenInfo, Self::ToBehaviour>,
    > {
        if let Some(event) = self.events.pop_front() {
            return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(event));
        }

        // Handle inbound reading
        match &mut self.inbound_state {
            StreamState::Idle => {
                if let Some(mut stream) = self.inbound_stream.take() {
                    info!("🔵 Starting inbound read");
                    let fut: ReadFuture = Box::pin(async move {
                        let msg = CoreLinkCodec::read_message(&mut stream).await?;
                        Ok((stream, msg))
                    });
                    self.inbound_state = StreamState::Reading(fut);
                }
            }
            StreamState::Reading(fut) => match fut.as_mut().poll(cx) {
                Poll::Ready(Ok((stream, msg))) => {
                    info!("📨 Received message: {:?}", msg.msg_type);
                    self.events
                        .push_back(CoreLinkHandlerEvent::MessageReceived(msg));
                    self.inbound_stream = Some(stream);
                    self.inbound_state = StreamState::Idle;
                    return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(
                        self.events.pop_front().unwrap(),
                    ));
                }
                Poll::Ready(Err(e)) => {
                    error!("❌ Failed to read message: {}", e);
                    self.inbound_state = StreamState::Idle;
                }
                Poll::Pending => {}
            },
            _ => {}
        }

        // Handle outbound writing
        match &mut self.outbound_state {
            StreamState::Idle => {
                if !self.pending_messages.is_empty() && self.can_request_outbound {
                    if self.outbound_stream.is_none() {
                        info!("🔴 Requesting outbound substream");
                        return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
                            protocol: SubstreamProtocol::new(CoreLinkProtocol, ()),
                        });
                    }

                    if let Some(mut stream) = self.outbound_stream.take() {
                        if let Some(msg) = self.pending_messages.pop_front() {
                            info!("🔴 Starting outbound write: {:?}", msg.msg_type);
                            let fut: WriteFuture = Box::pin(async move {
                                CoreLinkCodec::send_message(&mut stream, &msg).await?;
                                Ok(stream)
                            });
                            self.outbound_state = StreamState::Writing(fut);
                        }
                    }
                }
            }
            StreamState::Writing(fut) => match fut.as_mut().poll(cx) {
                Poll::Ready(Ok(stream)) => {
                    info!("📤 Sent message successfully");
                    self.events.push_back(CoreLinkHandlerEvent::MessageSent);
                    self.outbound_stream = Some(stream);
                    self.outbound_state = StreamState::Idle;
                }
                Poll::Ready(Err(e)) => {
                    error!("❌ Failed to send message: {}", e);
                    self.events
                        .push_back(CoreLinkHandlerEvent::SendError(e.to_string()));
                    self.outbound_state = StreamState::Idle;
                }
                Poll::Pending => {}
            },
            _ => {}
        }

        Poll::Pending
    }

    fn on_connection_event(
        &mut self,
        event: ConnectionEvent<
            Self::InboundProtocol,
            Self::OutboundProtocol,
            Self::InboundOpenInfo,
            Self::OutboundOpenInfo,
        >,
    ) {
        match event {
            ConnectionEvent::FullyNegotiatedInbound(stream) => {
                info!("🔵 Inbound stream fully negotiated");
                self.inbound_stream = Some(stream.protocol);
                // Allow outbound requests after inbound is established
                self.can_request_outbound = true;
            }
            ConnectionEvent::FullyNegotiatedOutbound(stream) => {
                info!("🔴 Outbound stream fully negotiated");
                self.outbound_stream = Some(stream.protocol);
                // Allow future outbound requests after one succeeds
                self.can_request_outbound = true;
            }
            ConnectionEvent::DialUpgradeError(err) => {
                self.dial_upgrade_failures += 1;
                if self.dial_upgrade_failures <= 2 {
                    info!(
                        "🔴 Dial upgrade failed (attempt {}): {:?}",
                        self.dial_upgrade_failures, err.error
                    );
                } else {
                    debug!(
                        "Dial upgrade failed (attempt {}): {:?}",
                        self.dial_upgrade_failures, err.error
                    );
                }

                // After 3 failures, stop trying and clear pending messages
                if self.dial_upgrade_failures >= 3 {
                    if !self.pending_messages.is_empty() {
                        debug!(
                            "Clearing {} pending messages due to repeated failures",
                            self.pending_messages.len()
                        );
                        self.pending_messages.clear();
                    }
                    self.can_request_outbound = false;
                }
            }
            ConnectionEvent::ListenUpgradeError(err) => {
                self.listen_upgrade_failures += 1;
                if self.listen_upgrade_failures <= 2 {
                    info!(
                        "🔵 Listen upgrade failed (attempt {}): {:?}",
                        self.listen_upgrade_failures, err.error
                    );
                } else {
                    debug!(
                        "Listen upgrade failed (attempt {}): {:?}",
                        self.listen_upgrade_failures, err.error
                    );
                }
            }
            _ => {}
        }
    }
}
