use std::error::Error;
use std::process::Output;

use bytes::Bytes;
use http::{HeaderMap, Request};
use tokio::net::TcpStream;

use h2::client;

use std::ops::Add;


#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    let _ = env_logger::try_init();

    let tcp = TcpStream::connect("127.0.0.1:5928").await?;
    let (mut client, h2, mut acceptor) = client::handshake(tcp).await?;

    println!("H2 connection bound, handshake success.");


    let job = tokio::spawn(async move {
        while let Some(result) = acceptor.accept().await {
            let (recv, respond) = result.unwrap();
            let b = String::from_utf8(recv.to_vec()).unwrap();
            if let Some(mut respond) = respond {
                println!("<<<< recv(NORMAL) {}", &b);
                respond.send_bifrost_call_response(Bytes::from(b)).unwrap();
                println!(">>>> send response");
            } else {
                println!("<<<< recv(ONE_SHOOT) {}", b);
            }
        }
    });


    println!("---sending non bifrost request---");

    let request = Request::builder()
        .uri("https://http2.akamai.com/")
        .body(())
        .unwrap();

    let mut trailers = HeaderMap::new();
    trailers.insert("zomg", "hello".parse().unwrap());

    let (response, mut stream) = client.send_request(request, false).unwrap();

    // send trailers
    stream.send_trailers(trailers).unwrap();

    // Spawn a task to run the conn...
    tokio::spawn(async move {
        if let Err(e) = h2.await {
            println!("GOT ERR={:?}", e);
        }
    });

    let response = response.await?;
    println!("GOT RESPONSE: {:?}", response);

    // Get the body
    let mut body = response.into_body();

    while let Some(chunk) = body.data().await {
        println!("GOT CHUNK = {:?}", chunk?);
    }

    if let Some(trailers) = body.trailers().await? {
        println!("GOT TRAILERS: {:?}", trailers);
    }

    let _ = job.await;
    Ok(())
}

#[cfg(test)]
mod test {
    use std::convert::TryFrom;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use bytes::Bytes;
    use futures_util::future::join_all;
    use http::Request;
    use tokio::net::TcpStream;
    use h2::client;


    #[tokio::test]
    async fn test_client() {
        let cnt: AtomicUsize = AtomicUsize::new(0);

        let _ = env_logger::try_init();
        // let tcp = Rc::new(TcpStream::connect("127.0.0.1:5928").await?);

        let mut clients = Vec::new();
        for i in 0..10 {

            //handshake要求获得：所有权
            let conn = TcpStream::connect("127.0.0.1:5928").await.unwrap();

            let (mut client, h2, mut acceptor) = client::handshake(conn).await.unwrap();


            /// Connection<TcpStream> 是一个驱动器，你可以理解为Netty的EventLoop的协程版本，你需要为他创建一个task并运行起来。
            /// 它会负责出入。
            ///
            /// 注意观察，不同对象所有权被不同东西捕获了。
            tokio::spawn(async move{
                let _ = h2.await;
            });

            clients.push((client, acceptor));
        }

        let mut jobs = Vec::new();

        for i in 0..10 {
            let request = Request::builder()
                .uri("https://baidu.com/")
                .body(())
                .unwrap();
            let c = cnt.fetch_add(1, Ordering::Relaxed);
            let  (client, acceptor) = &mut clients[c % 10];


            let (future, mut stream) = client.send_request(request, false).unwrap();
            stream.send_data(Bytes::from_static(b"123"), true);
            let job = tokio::spawn(async move {
                let x = future.await;
                dbg!(x);
                // println!("get response{}", response)
            });
            jobs.push(job)
        }
        join_all(jobs).await;
    }
}