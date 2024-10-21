mod bot;
mod commands;
use log::{error, info};
use poise::Framework;
use serenity::all::ClientBuilder;
use songbird::SerenityInit;
use spoticord_database::Database;
use actix_web::{web, App, HttpResponse, HttpServer};
use std::sync::Arc;
use tokio::sync::Mutex;

async fn hello() -> HttpResponse {
    HttpResponse::Ok().body("Hello World from Spoticord!")
}

#[tokio::main]
async fn main() {
    // Force aws-lc-rs as default crypto provider
    // Since multiple dependencies either enable aws_lc_rs or ring, they cause a clash, so we have to
    // explicitly tell rustls to use the aws-lc-rs provider
    * = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // Setup logging
    if std::env::var("RUST_LOG").is_err() {
        #[cfg(debug_assertions)]
        std::env::set_var("RUST_LOG", "spoticord,actix_web=info");
        #[cfg(not(debug_assertions))]
        std::env::set_var("RUST_LOG", "spoticord=info,actix_web=info");
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
        .setup(|ctx, ready, framework| Box::pin(bot::setup(ctx, ready, framework, database.clone())))
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

    // Start HTTP server
    let http_server = HttpServer::new(move || {
        App::new().route("/", web::get().to(hello))
    })
    .bind("127.0.0.1:10000")
    .expect("Can not bind to port 10000");

    // Run bot and HTTP server concurrently
    let bot_task = tokio::spawn(async move {
        if let Err(why) = client.start_autosharded().await {
            error!("Fatal error occurred during bot operations: {why}");
            error!("Bot will now shut down!");
        }
    });

    let http_task = tokio::spawn(async move {
        info!("HTTP server running at http://localhost:10000");
        http_server.run().await.expect("HTTP server failed");
    });

    // Wait for both tasks to complete
    tokio::try_join!(bot_task, http_task).expect("Failed to run services");
}
