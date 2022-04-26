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
    use std::sync::atomic::{AtomicI64, Ordering};
    use http::Request;
    use tokio::net::TcpStream;
    use h2::client;


    #[tokio::test]
    async fn test_client() {
        let cnt: AtomicI64 = AtomicI64::new(0);

        let _ = env_logger::try_init();
        let tcp = Rc::new(TcpStream::connect("127.0.0.1:5928").await?);

        let mut connections = Arc::new(Vec::new());
        for i in 0..10 {
            let (mut client, h2, mut acceptor) = client::handshake(*Rc::clone(&tcp)).await?;
            connections.push((client, h2, acceptor));
        }

        let request = Arc::new(Request::builder()
            .uri("https://baidu.com/")
            .body(String::from("123123123"))
            .unwrap());

        for i in 0..100000 {
            tokio::spawn(async move {
                let cnt = usize::try_from(cnt.fetch_add(1, Ordering::Relaxed) % 10).unwrap();
                let conns = Arc::clone(&connections);

                let (client, h2, acceptor) = *conns[cnt];
                let (response, mut stream) = client.send_request(Arc::clone(&request), true).unwrap();
                let response = response.await?.into_body();
                println!("get response{}", response)
            });
        }
    }
}