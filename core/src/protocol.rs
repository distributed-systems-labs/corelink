use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p_core::upgrade::{InboundUpgrade, OutboundUpgrade, UpgradeInfo};
use libp2p_swarm::StreamProtocol;
use std::io;

#[derive(Debug, Clone)]
pub struct CoreLinkProtocol;

impl UpgradeInfo for CoreLinkProtocol {
    type Info = StreamProtocol;
    type InfoIter = std::iter::Once<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        std::iter::once(StreamProtocol::new("/corelink/1.0.0"))
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
        futures::future::ok(socket)
    }
}

#[derive(Debug, Clone)]
pub struct CoreLinkCodec;

impl CoreLinkCodec {
    pub async fn send_message<T>(stream: &mut T, msg: &crate::Message) -> io::Result<()>
    where
        T: AsyncWrite + Unpin,
    {
        let json = serde_json::to_string(msg)?;
        let len = json.len() as u32;

        stream.write_all(&len.to_be_bytes()).await?;
        stream.write_all(json.as_bytes()).await?;
        stream.flush().await?;

        Ok(())
    }

    pub async fn read_message<T>(stream: &mut T) -> io::Result<crate::Message>
    where
        T: AsyncRead + Unpin,
    {
        let mut len_bytes = [0u8; 4];
        stream.read_exact(&mut len_bytes).await?;
        let len = u32::from_be_bytes(len_bytes) as usize;

        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf).await?;

        let msg = serde_json::from_slice(&buf)?;
        Ok(msg)
    }
}
