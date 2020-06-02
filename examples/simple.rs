use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use simple_socket::{PostServing, SocketClient, SocketServer};

use podo_core_driver::AliveFlag;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
enum Request {
    Hello(String),
    Goodbye(String),
}

#[derive(Debug, Serialize, Deserialize)]
enum Response {
    Echo(String),
}

fn main() {
    const IP_V4: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);
    const IP: IpAddr = IpAddr::V4(IP_V4);
    const PORT: u16 = 9804;

    let socket = SocketAddr::new(IP, PORT);

    let alive = AliveFlag::new(true);
    let active = AliveFlag::new(false);

    let server_alive = alive.clone();
    let server_active = active.clone();
    let server_thread = std::thread::spawn(move || {
        let handler = |req| match req {
            Request::Hello(name) => Response::Echo(format!("Hello, {}!", name)),
            Request::Goodbye(name) => Response::Echo(format!("Goodbye, {}!", name)),
        };

        let backlog = Default::default();
        let server = SocketServer::<Request, Response>::try_new(socket, backlog).unwrap();
        server
            .run(handler, |server| {
                server_active.start().ok();
                if server_alive.is_running() || server.has_connections() {
                    PostServing::Yield
                } else {
                    PostServing::Stop
                }
            })
            .unwrap();
    });

    while !active.is_running() {
        std::thread::yield_now();
    }

    {
        let name = "foo".to_string();

        let mut client = SocketClient::<Request, Response>::try_new(socket).unwrap();

        let response = client.request(&Request::Hello(name.clone())).unwrap();
        dbg!(response);

        let response = client.request(&Request::Goodbye(name)).unwrap();
        dbg!(response);
    }

    alive.stop().unwrap();
    server_thread.join().unwrap();
}
