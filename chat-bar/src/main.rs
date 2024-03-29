use once_cell::sync::OnceCell;
use std::{error::Error, time::Duration};
use tokio::{io, io::AsyncBufReadExt};
use tracing::debug;
use tracing_subscriber::EnvFilter;

mod p2p;
mod ui;
use p2p::evt_loop;
mod msg;
use msg::*;

const TITLE: &str = include_str!("./title.txt");

fn global_rt() -> &'static tokio::runtime::Runtime {
    static RT: OnceCell<tokio::runtime::Runtime> = OnceCell::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap())
}

fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let mut app = ui::App::default();
    for line in TITLE.lines() {
        app.add_message(
            Msg::default()
                .set_content(line.to_string())
                .set_kind(MsgKind::Raw),
        );
    }

    let (peer_tx, mut peer_rx) = tokio::sync::mpsc::channel::<Msg>(100);
    let (input_tx, input_rx) = tokio::sync::mpsc::channel::<Msg>(100);

    // let input_loop_fut = input_loop(input_tx);
    let input_tx_clone = input_tx.clone();
    app.on_submit(move |m| {
        debug!("sent: {:?}", m);
        input_tx_clone.blocking_send(m).unwrap();
    });

    global_rt().spawn(async move {
        evt_loop(input_rx, peer_tx).await.unwrap();
    });

    // recv from peer
    let mut tui_msg_adder = app.add_msg_fn();
    global_rt().spawn(async move {
        while let Some(m) = peer_rx.recv().await {
            debug!("recv: {:?}", m);
            tui_msg_adder(m);
        }
    });

    // say hi
    let input_tx_clone = input_tx.clone();
    global_rt().spawn(async move {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        input_tx_clone
            .send(Msg::default().set_kind(MsgKind::Join))
            .await
            .unwrap();
    });

    app.run()?;

    // say goodbye
    input_tx.blocking_send(Msg::default().set_kind(MsgKind::Leave))?;
    std::thread::sleep(Duration::from_millis(500));

    Ok(())
}

async fn input_loop(self_input: tokio::sync::mpsc::Sender<Vec<u8>>) -> Result<(), Box<dyn Error>> {
    let mut stdin = io::BufReader::new(io::stdin()).lines();
    while let Some(line) = stdin.next_line().await? {
        let msg = Msg::default().set_content(line);
        if let Ok(b) = serde_json::to_vec(&msg) {
            self_input.send(b).await?;
        }
    }
    Ok(())
}
