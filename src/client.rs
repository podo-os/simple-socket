use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::net::{Shutdown, SocketAddr};
use std::sync::{Mutex, MutexGuard};

use bincode::Result;
use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};
use serde::{de::DeserializeOwned, Serialize};
use socket2::{Domain, Socket, Type};

pub struct SocketClient<Req, Res>
where
    Req: Serialize,
    Res: DeserializeOwned,
{
    buffer: Mutex<Buffer>,
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
            buffer: Mutex::default(),
            stream: socket,

            _request: PhantomData::default(),
            _response: PhantomData::default(),
        })
    }

    pub(crate) fn try_from_stream(stream: Socket) -> io::Result<Self> {
        stream.set_nonblocking(true)?;

        Ok(Self {
            buffer: Mutex::default(),
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
    pub fn request(&self, request: &Req) -> Result<Res> {
        let mut buffer = self.buffer.lock().unwrap();

        let stream = &mut &self.stream;

        buffer.data.clear();
        bincode::serialize_into(&mut buffer.data, request)?;

        let size = buffer.data.len() as u64;
        assert_ne!(size, 0, "Message must have one or more bytes.");

        stream.write_u64::<NetworkEndian>(size)?;
        stream.write_all(&buffer.data)?;

        bincode::deserialize_from(stream)
    }

    pub(crate) fn response<F>(&self, handler: F) -> Result<SocketStatus>
    where
        F: FnMut(Res) -> Req,
    {
        let mut buffer = self.buffer.lock().unwrap();

        let stream = &mut &self.stream;

        if buffer.size.is_some() {
            fill_buffer_and_handle(buffer, stream, handler)
        } else {
            let mut buf = [0; 8];
            match stream.peek(&mut buf) {
                Ok(8) => {
                    stream.read_exact(&mut buf)?;

                    let size = buf.as_ref().read_u64::<NetworkEndian>()? as usize;
                    if size == 0 {
                        return Ok(SocketStatus::Closed);
                    }

                    buffer.offset = 0;
                    buffer.size = Some(size);
                    buffer.data.resize(size, 0);
                    fill_buffer_and_handle(buffer, stream, handler)
                }
                Ok(_) => Ok(SocketStatus::Alive),
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(SocketStatus::Alive),
                Err(error) => Err(error.into()),
            }
        }
    }

    fn stop(&self) -> io::Result<()> {
        let stream = &mut &self.stream;

        stream.write_u64::<NetworkEndian>(0)?;
        stream.shutdown(Shutdown::Read)?;
        Ok(())
    }
}

fn fill_buffer_and_handle<Res, Req, F>(
    mut buffer: MutexGuard<Buffer>,
    stream: &mut &Socket,
    mut handler: F,
) -> Result<SocketStatus>
where
    Req: Serialize,
    Res: DeserializeOwned,
    F: FnMut(Res) -> Req,
{
    let offset = buffer.offset;

    buffer.offset += match stream.read(&mut buffer.data[offset..]) {
        Ok(size) => size,
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(SocketStatus::Alive),
        Err(error) => return Err(error.into()),
    };
    if buffer.offset >= buffer.size.unwrap() {
        let request = bincode::deserialize_from(&buffer.data[..])?;
        buffer.size = None;

        bincode::serialize_into(stream, &handler(request))?;
        Ok(SocketStatus::Alive)
    } else {
        Ok(SocketStatus::Alive)
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

#[derive(Default)]
struct Buffer {
    data: Vec<u8>,
    offset: usize,
    size: Option<usize>,
}
