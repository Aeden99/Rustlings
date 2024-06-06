//I have majorly followed the example for the single threaded web server
//given on hyper crate's github documentation.
//Due to insufficient knowledge of alternatives of some data types and functions here,
//as well as their crucialness, I have not made any changes to some parts of their code.

use std::cell::Cell;
use std::rc::Rc;
use tokio::net::TcpListener;

use hyper::body::{Body as HttpBody, Bytes, Frame};
use hyper::service::service_fn;
use hyper::{Error, Response};
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::thread;
use tokio::net::TcpStream;
mod support;
use support::TokioIo;
use tokio::signal;
use tokio::sync::oneshot;

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
    let (shutdown_tx,shutdown_rx)=oneshot::channel();
    let (shutdown_cx,shutdown_dx)=oneshot::channel();
    let server_http1 = thread::spawn(move || {
        let rntm = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("runtime could not be built");
        let local = tokio::task::LocalSet::new();
        local.block_on(&rntm, http1_server(shutdown_rx,shutdown_dx)).unwrap();
    });
    let shutdown_thread=thread::spawn(move ||{
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            signal::ctrl_c().await.expect("shutdown signal not received");
            println!("Shutting Down Server");
            shutdown_tx.send(()).unwrap();
        })});
    let shutdown_thread_2=thread::spawn(move ||{
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            signal::ctrl_c().await.expect("shutdown signal not received");
            println!("Shutting Down Runtimes");
            shutdown_cx.send(()).unwrap();
        })});
    server_http1.join().unwrap();
    shutdown_thread.join().unwrap();
    shutdown_thread_2.join().unwrap();
}
async fn http1_server(shutdown_rx:oneshot::Receiver<()>,shutdown_dx:oneshot::Receiver<()>) -> Result<(), Box<dyn std::error::Error>> {

    let listener = TcpListener::bind(("127.0.0.1",7878)).await?;
    let counter = Rc::new(Cell::new(0));
    let shutdown=async{
        shutdown_rx.await.ok();
    };
    let shutdown_2=async{
        shutdown_dx.await.ok();
    };
    tokio::select!{
        _=shutdown=>{
            println!("Shutting Down All Threads");
        },
        _=shutdown_2=>{
            println!("Shutting Down All Threads");
        },
    }
    loop {
        let (stream, socket) = listener.accept().await?;

        let io = IOTypeNotSend::new(TokioIo::new(stream));

        let cnt = counter.clone();
        //This is a static type allocation allowing parallel connections to have different counters

        let service = service_fn(move |_| {
            let prev = cnt.get();
            cnt.set(prev + 1);
            let value = cnt.get();
            async move { Ok::<_, Error>(Response::new(Body::from(format!("Successful Connection No.{} from Address {:?}", value,socket)))) }
        });

        tokio::task::spawn_local(async move {
            if let Err(err) = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, service)
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
struct IOTypeNotSend {
    _marker: PhantomData<*const ()>,
    stream: TokioIo<TcpStream>,
}

impl IOTypeNotSend {
    fn new(stream: TokioIo<TcpStream>) -> Self {
        Self {
            _marker: PhantomData,
            stream,
        }
    }
}

impl hyper::rt::Write for IOTypeNotSend {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<() , std::io::Error>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

impl hyper::rt::Read for IOTypeNotSend {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}