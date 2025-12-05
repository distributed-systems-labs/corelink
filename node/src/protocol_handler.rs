use corelink_core::Message;
use futures::{AsyncRead, AsyncWrite};
use libp2p_core::upgrade::{InboundUpgrade, OutboundUpgrade, UpgradeInfo};
use libp2p_swarm::{
    handler::ConnectionEvent, ConnectionHandler, ConnectionHandlerEvent,
    SubstreamProtocol, StreamProtocol,
};
use std::collections::VecDeque;
use std::io;
use std::task::{Context, Poll};
use tracing::debug;

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
        debug!("Inbound protocol upgrade");
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
        debug!("Outbound protocol upgrade");
        futures::future::ok(socket)
    }
}

#[derive(Debug)]
pub enum CoreLinkHandlerEvent {
    MessageReceived(Message),
    MessageSent,
}

pub struct CoreLinkHandler {
    pending_messages: VecDeque<Message>,
    events: VecDeque<CoreLinkHandlerEvent>,
}

impl CoreLinkHandler {
    pub fn new() -> Self {
        Self {
            pending_messages: VecDeque::new(),
            events: VecDeque::new(),
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
        debug!("Handler received message from behaviour");
        self.pending_messages.push_back(message);
    }

    fn poll(
        &mut self,
        _cx: &mut Context,
    ) -> Poll<ConnectionHandlerEvent<Self::OutboundProtocol, Self::OutboundOpenInfo, Self::ToBehaviour>> {
        if let Some(event) = self.events.pop_front() {
            return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(event));
        }

        Poll::Pending
    }

    fn on_connection_event(
        &mut self,
        event: ConnectionEvent<Self::InboundProtocol, Self::OutboundProtocol, Self::InboundOpenInfo, Self::OutboundOpenInfo>,
    ) {
        match event {
            ConnectionEvent::FullyNegotiatedInbound(_stream) => {
                debug!("Inbound stream fully negotiated");
            }
            ConnectionEvent::FullyNegotiatedOutbound(_stream) => {
                debug!("Outbound stream fully negotiated");
            }
            _ => {}
        }
    }
}