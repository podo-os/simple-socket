use std::io;
use std::net::SocketAddr;

use super::message::{Message, Response};
use crate::client::SocketClient;

use bincode::Result;
use serde::{de::DeserializeOwned, Serialize};

pub struct ActorClient<Req, Res>
where
    Self: Send + Sync,
    Req: Serialize + PartialEq + Eq + Send + Sync,
    Res: DeserializeOwned + PartialEq + Eq + Send + Sync,
{
    inner: SocketClient<Message<Req>, Response<Res>>,
}

impl<Req, Res> ActorClient<Req, Res>
where
    Self: Send + Sync,
    Req: Serialize + PartialEq + Eq + Send + Sync,
    Res: DeserializeOwned + PartialEq + Eq + Send + Sync,
{
    pub fn try_new(addr: SocketAddr) -> io::Result<Self> {
        Ok(Self {
            inner: SocketClient::try_new(addr)?,
        })
    }

    pub fn stop(mut self) -> Result<()> {
        self.inner.request(&Message::Stop).map(|_| ())
    }

    pub fn pause(&mut self) -> Result<()> {
        self.inner.request(&Message::Pause).map(|_| ())
    }

    pub fn resume(&mut self) -> Result<()> {
        self.inner.request(&Message::Resume).map(|_| ())
    }

    pub fn hibernate(&mut self) -> Result<()> {
        self.inner.request(&Message::Hibernate).map(|_| ())
    }

    pub fn wake_up(&mut self) -> Result<()> {
        self.inner.request(&Message::WakeUp).map(|_| ())
    }

    pub fn request(&mut self, message: Req) -> Result<Res> {
        self.inner
            .request(&Message::Custom(message))
            .map(|m| m.unwrap_custom())
    }
}
