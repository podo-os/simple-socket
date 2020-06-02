use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::net::{Shutdown, SocketAddr};

use bincode::Result;
use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};
use serde::{de::DeserializeOwned, Serialize};
use socket2::{Domain, Socket, Type};

pub struct SocketClient<Req, Res>
where
    Req: Serialize,
    Res: DeserializeOwned,
{
    buffer: Vec<u8>,
    buffer_offset: usize,
    buffer_size: Option<usize>,
    stream: Socket,

    _request: PhantomData<Req>,
    _response: PhantomData<Res>,
}

impl<Req, Res> SocketClient<Req, Res>
where
    Req: Serialize,
    Res: DeserializeOwned,
{
    pub fn try_new(addr: SocketAddr) -> io::Result<Self> {
        let domain = match addr {
            SocketAddr::V4(_) => Domain::ipv4(),
            SocketAddr::V6(_) => Domain::ipv6(),
        };

        let socket = Socket::new(domain, Type::stream(), None)?;
        socket.connect(&addr.into())?;

        Ok(Self {
            buffer: vec![],
            buffer_offset: 0,
            buffer_size: None,
            stream: socket,

            _request: PhantomData::default(),
            _response: PhantomData::default(),
        })
    }

    pub(crate) fn try_from_stream(stream: Socket) -> io::Result<Self> {
        stream.set_nonblocking(true)?;

        Ok(Self {
            buffer: vec![],
            buffer_offset: 0,
            buffer_size: None,
            stream,

            _request: PhantomData::default(),
            _response: PhantomData::default(),
        })
    }
}

impl<Req, Res> SocketClient<Req, Res>
where
    Req: Serialize,
    Res: DeserializeOwned,
{
    pub fn request(&mut self, request: &Req) -> Result<Res> {
        self.buffer.clear();
        bincode::serialize_into(&mut self.buffer, request)?;

        let size = self.buffer.len() as u64;
        assert_ne!(size, 0, "Message must have one or more bytes.");

        self.stream.write_u64::<NetworkEndian>(size)?;
        self.stream.write_all(&self.buffer)?;

        bincode::deserialize_from(&mut self.stream)
    }

    pub(crate) fn response<F>(&mut self, handler: F) -> Result<SocketStatus>
    where
        F: FnMut(Res) -> Req,
    {
        if self.buffer_size.is_some() {
            self.fill_buffer_and_handle(handler)
        } else {
            let mut buf = [0; 8];
            match self.stream.peek(&mut buf) {
                Ok(8) => {
                    self.stream.read_exact(&mut buf)?;

                    let size = buf.as_ref().read_u64::<NetworkEndian>()? as usize;
                    if size == 0 {
                        return Ok(SocketStatus::Closed);
                    }

                    self.set_buffer(size);
                    self.fill_buffer_and_handle(handler)
                }
                Ok(_) => Ok(SocketStatus::Alive),
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(SocketStatus::Alive),
                Err(error) => Err(error.into()),
            }
        }
    }

    fn stop(&mut self) -> io::Result<()> {
        self.stream.write_u64::<NetworkEndian>(0)?;
        self.stream.shutdown(Shutdown::Read)?;
        Ok(())
    }

    fn set_buffer(&mut self, size: usize) {
        self.buffer_offset = 0;
        self.buffer_size = Some(size);
        self.buffer.resize(size, 0);
    }

    fn reset_buffer(&mut self) {
        self.buffer_size = None;
    }

    fn fill_buffer_and_handle<F>(&mut self, mut handler: F) -> Result<SocketStatus>
    where
        F: FnMut(Res) -> Req,
    {
        self.buffer_offset += match self.stream.read(&mut self.buffer[self.buffer_offset..]) {
            Ok(size) => size,
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(SocketStatus::Alive),
            Err(error) => return Err(error.into()),
        };
        if self.buffer_offset >= self.buffer_size.unwrap() {
            let request = bincode::deserialize_from(&self.buffer[..])?;
            self.reset_buffer();

            bincode::serialize_into(&mut self.stream, &handler(request))?;
            Ok(SocketStatus::Alive)
        } else {
            Ok(SocketStatus::Alive)
        }
    }
}

impl<Req, Res> Drop for SocketClient<Req, Res>
where
    Req: Serialize,
    Res: DeserializeOwned,
{
    fn drop(&mut self) {
        match self.stop() {
            Ok(()) => (),
            Err(ref e) if e.kind() == io::ErrorKind::NotConnected => (),
            Err(error) => panic!(error),
        }
    }
}

pub enum SocketStatus {
    Alive,
    Closed,
}
