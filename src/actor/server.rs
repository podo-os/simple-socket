use std::cell::UnsafeCell;
use std::net::SocketAddr;

use super::message::{Message, Response};
use crate::server::{PostServing, SocketServer};

use podo_core_driver::RuntimeError;
use serde::{de::DeserializeOwned, Serialize};

pub trait ActorServer<Req, Res>
where
    Self: Send + Sync,
    Req: DeserializeOwned + PartialEq + Eq + Send + Sync,
    Res: Serialize + PartialEq + Eq + Send + Sync,
{
    fn step(&mut self) -> Result<(), RuntimeError>;

    fn stop(self) -> Result<(), RuntimeError>;

    fn pause(&mut self) -> Result<(), RuntimeError>;
    fn resume(&mut self) -> Result<(), RuntimeError>;

    fn hibernate(&mut self) -> Result<(), RuntimeError>;
    fn wake_up(&mut self) -> Result<(), RuntimeError>;

    fn request(&mut self, message: Req) -> Result<Res, RuntimeError>;

    fn run_server(self, socket: SocketAddr)
    where
        Self: Sized,
    {
        let actor = UnsafeCell::new(self);
        let get_actor = || unsafe { actor.get().as_mut().unwrap() };

        let alive = UnsafeCell::new(true);
        let get_alive = || unsafe { alive.get().as_mut().unwrap() };

        let handler = |req| match req {
            Message::Stop => {
                *get_alive() = false;
                Response::Awk
            }
            Message::Pause => {
                get_actor().pause().unwrap();
                Response::Awk
            }
            Message::Resume => {
                get_actor().resume().unwrap();
                Response::Awk
            }
            Message::Hibernate => {
                get_actor().hibernate().unwrap();
                Response::Awk
            }
            Message::WakeUp => {
                get_actor().wake_up().unwrap();
                Response::Awk
            }
            Message::Custom(m) => Response::Custom(get_actor().request(m).unwrap()),
        };

        let backlog = Default::default();
        let server = SocketServer::<Message<Req>, Response<Res>>::try_new(socket, backlog).unwrap();
        server
            .run(handler, |_| {
                get_actor().step().unwrap();

                if *get_alive() {
                    PostServing::Yield
                } else {
                    PostServing::Stop
                }
            })
            .unwrap();

        actor.into_inner().stop().unwrap();
    }
}
