mod bot;
mod commands;
use log::{error, info};
use poise::Framework;
use serenity::all::ClientBuilder;
use songbird::SerenityInit;
use spoticord_database::Database;
use std::net::{TcpStream, TcpListener};
use std::io::{Read, Write};
use std::thread;

fn handle_read(mut stream: &TcpStream) {
    let mut buf = [0u8 ;4096];
    match stream.read(&mut buf) {
        Ok(_) => {
            let req_str = String::from_utf8_lossy(&buf);
            println!("{}", req_str);
        },
        Err(e) => println!("Unable to read stream: {}", e),
    }
}

fn handle_write(mut stream: TcpStream) {
    let response = b"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n<html><body>Hello world</body></html>\r\n";
    match stream.write(response) {
        Ok(_) => println!("Response sent"),
        Err(e) => println!("Failed sending response: {}", e),
    }
}

fn handle_client(stream: TcpStream) {
    handle_read(&stream);
    handle_write(stream);
}

#[tokio::main]
async fn main() {
    // Force aws-lc-rs as default crypto provider
    // Since multiple dependencies either enable aws_lc_rs or ring, they cause a clash, so we have to
    // explicitly tell rustls to use the aws-lc-rs provider
    * = rustls::crypto::aws*lc_rs::default_provider().install_default();
    // Setup logging
    if std::env::var("RUST_LOG").is_err() {
        #[cfg(debug_assertions)]
        std::env::set_var("RUST_LOG", "spoticord");
        #[cfg(not(debug_assertions))]
        std::env::set_var("RUST_LOG", "spoticord=info");
    }
    env_logger::init();
    info!("Today is a good day!");
    info!(" - Spoticord");
    dotenvy::dotenv().ok();
    // Set up database
    let database = match Database::connect().await {
        Ok(db) => db,
        Err(why) => {
            error!("Failed to connect to database and perform migrations: {why}");
            return;
        }
    };
    // Set up bot
    let framework = Framework::builder()
        .setup(|ctx, ready, framework| Box::pin(bot::setup(ctx, ready, framework, database)))
        .options(bot::framework_opts())
        .build();
    let mut client = match ClientBuilder::new(
        spoticord_config::discord_token(),
        spoticord_config::discord_intents(),
    )
    .framework(framework)
    .register_songbird_from_config(songbird::Config::default().use_softclip(false))
    .await
    {
        Ok(client) => client,
        Err(why) => {
            error!("Fatal error when building Serenity client: {why}");
            return;
        }
    };

    // Start the HTTP server in a separate thread
    thread::spawn(|| {
        let listener = TcpListener::bind("0.0.0.0:8080").unwrap();
        println!("Listening for connections on port {}", 8080);
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    thread::spawn(|| {
                        handle_client(stream)
                    });
                }
                Err(e) => {
                    println!("Unable to connect: {}", e);
                }
            }
        }
    });

    if let Err(why) = client.start_autosharded().await {
        error!("Fatal error occured during bot operations: {why}");
        error!("Bot will now shut down!");
    }
}
