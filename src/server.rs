use std::io;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::time::Duration;

use crate::backlog::Backlog;
use crate::client::{SocketClient, SocketStatus};

use bincode::Result;
use serde::{de::DeserializeOwned, Serialize};
use socket2::{Domain, Socket, Type};

pub struct SocketServer<Req, Res>
where
    Req: DeserializeOwned,
    Res: Serialize,
{
    streams: Vec<SocketClient<Res, Req>>,
    listener: Socket,

    _request: PhantomData<Req>,
    _response: PhantomData<Res>,
}

impl<Req, Res> SocketServer<Req, Res>
where
    Req: DeserializeOwned,
    Res: Serialize,
{
    pub fn try_new(addr: SocketAddr, backlog: Backlog) -> io::Result<Self> {
        let domain = match addr {
            SocketAddr::V4(_) => Domain::ipv4(),
            SocketAddr::V6(_) => Domain::ipv6(),
        };

        let socket = Socket::new(domain, Type::stream(), None)?;
        socket.bind(&addr.into())?;
        socket.listen(backlog.into())?;

        socket.set_nonblocking(true)?;

        Ok(Self {
            streams: vec![],
            listener: socket,

            _request: PhantomData::default(),
            _response: PhantomData::default(),
        })
    }
}

impl<Req, Res> SocketServer<Req, Res>
where
    Req: DeserializeOwned,
    Res: Serialize,
{
    pub fn run<H, P>(mut self, mut handler: H, post: P) -> Result<()>
    where
        H: FnMut(Req) -> Res,
        P: Fn(&mut Self) -> PostServing,
    {
        loop {
            if let Some(server_client) = self.accept()? {
                self.streams.push(server_client);
            }
            for idx in (0..self.streams.len()).rev() {
                let client = &mut self.streams[idx];

                if let SocketStatus::Closed = client.response(|req| handler(req))? {
                    self.streams.remove(idx);
                }
            }
            match post(&mut self) {
                PostServing::Wait(time) => std::thread::sleep(time),
                PostServing::Yield => std::thread::yield_now(),
                PostServing::Continue => continue,
                PostServing::Stop => break Ok(()),
            }
        }
    }

    pub fn has_connections(&self) -> bool {
        !self.streams.is_empty()
    }

    pub fn num_connections(&self) -> usize {
        self.streams.len()
    }

    fn accept(&mut self) -> io::Result<Option<SocketClient<Res, Req>>> {
        match self.listener.accept() {
            Ok((stream, _)) => Ok(Some(SocketClient::try_from_stream(stream)?)),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error),
        }
    }
}

pub enum PostServing {
    Wait(Duration),
    Yield,
    Continue,
    Stop,
}
