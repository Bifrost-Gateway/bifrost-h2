use h2::client;
use http::{HeaderMap, Request};

use std::error::Error;
use bytes::Bytes;

use tokio::net::TcpStream;

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
            }else {
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
