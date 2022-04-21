use std::error::Error;

use bytes::Bytes;
use h2::server::{self, SendResponse};
use h2::RecvStream;
use http::Request;
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let _ = env_logger::try_init();

    let listener = TcpListener::bind("127.0.0.1:5928").await?;

    println!("Bifrost server bootup, listening on {:?}", listener.local_addr());

    loop {
        if let Ok((socket, _peer_addr)) = listener.accept().await {
            tokio::spawn(async move {
                if let Err(e) = serve(socket).await {
                    println!("  -> err={:?}", e);
                }
            });
        }
    }
}

async fn serve(socket: TcpStream) -> Result<(), Box<dyn Error + Send + Sync>> {
    let (mut connection, mut call_sender) = server::handshake(socket).await?;
    println!("H2 connection bound, handshake success.");

    // for receive non bifrost request
    tokio::spawn(async move {
        while let Some(result) = connection.accept().await {
            let (request, respond) = result.unwrap();
            tokio::spawn(async move {
                if let Err(e) = handle_non_bifrost_request(request, respond).await {
                    println!("error while handling request: {}", e);
                }
            });
        }
    });

    //for send NORMAL bifrost request
    tokio::spawn(async move {
        for _i in 0..10 {
            let response = call_sender.send_bifrost_call(Bytes::from(_i.to_string()), false).await.unwrap().unwrap();
            println!(">>>> send(NORMAL): {}", _i.to_string());
            let r = response.await.unwrap();
            let s = String::from_utf8(r.to_vec()).unwrap();
            println!("<<<< recv {}", s);
        }

        for _i in 10..20 {
            if let None = call_sender.send_bifrost_call(Bytes::from(_i.to_string()), true).await.unwrap() {
                println!(">>>> send(ONE_SHOOT): {}", _i.to_string());
            }
        }
    });


    Ok(())
}

async fn handle_non_bifrost_request(
    mut request: Request<RecvStream>,
    mut respond: SendResponse<Bytes>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("GOT request: {:?}", request);

    let body = request.body_mut();
    while let Some(data) = body.data().await {
        let data = data?;
        println!("<<<< recv {:?}", data);
        let _ = body.flow_control().release_capacity(data.len());
    }

    let response = http::Response::new(());
    let mut send = respond.send_response(response, false)?;
    send.send_data(Bytes::from_static(b"hello "), false)?;
    send.send_data(Bytes::from_static(b"world\n"), true)?;

    Ok(())
}
