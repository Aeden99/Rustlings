//I have majorly followed the example for the single threaded web server
//given on hyper crate's github documentation.
//Due to insufficient knowledge of alternatives of some data types and functions here,
//as well as their crucialness, I have not made any changes to some parts of their code.
//Rest assured I have tried my best to understand the code and not copy paste here.

#![deny(warnings)]

use http_body_util::BodyExt;
use std::cell::Cell;
use std::net::SocketAddr;
use std::rc::Rc;
use tokio::io::{self, AsyncWriteExt};
use tokio::net::TcpListener;

use hyper::body::{Body as HttpBody, Bytes, Frame};
use hyper::service::service_fn;
use hyper::Request;
use hyper::{Error, Response};
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::thread;
use tokio::net::TcpStream;
mod support;
use support::TokioIo;

struct Body {
    marker: PhantomData<*const ()>,
    data: Option<Bytes>,
}

impl From<String> for Body {
    fn from(a: String) -> Self {
        Body {
            marker: PhantomData,
            data: Some(a.into()),
        }
    }
}

impl HttpBody for Body {
    type Data = Bytes;
    type Error = Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Poll::Ready(self.get_mut().data.take().map(|d| Ok(Frame::data(d))))
    }
}

fn main(){
    pretty_env_logger::init();
    let server_http1 = thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build runtime");
        let local = tokio::task::LocalSet::new();
        local.block_on(&rt, http1_server()).unwrap();
    });

    let client_http1 = thread::spawn(move || {
        // Configure a runtime for the client that runs everything on the current thread
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build runtime");

        // Combine it with a `LocalSet,  which means it can spawn !Send futures...
        let local = tokio::task::LocalSet::new();
        local
            .block_on(
                &rt,
                http1_client("http://localhost:3001".parse::<hyper::Uri>().unwrap()),
            )
            .unwrap();
    });

    server_http1.join().unwrap();
    client_http1.join().unwrap();
}
