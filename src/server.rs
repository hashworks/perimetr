mod database;
mod helper;
mod models;
mod services;

use services::layer;

use actix_cors::Cors;
use actix_files as fs;
use actix_web::{
    middleware::{self, Logger},
    web, App, HttpServer,
};
use clap::{value_parser, Arg, Command};
use env_logger::Env;
use sqlx::postgres::PgPoolOptions;
use std::path::PathBuf;

#[derive(Clone)]
struct Configuration {
    layer_path: PathBuf,
    layer_suffix: String,
}

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let matches = Command::new("perimetr-server")
        .about("Webservice that accepts VSSS shares for perimetr layers and decrypts them when enough shares are received.")
        .arg_required_else_help(false)
        .arg(
            Arg::new("layer-path")
                .short('p')
                .long("layer-path")
                .help("Path to layer files")
                .required(false)
                .default_value(".")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("layer-suffix")
                .short('s')
                .long("layer-suffix")
                .help("Suffix of layer files")
                .required(false)
                .default_value(".layer.yml")
                .value_parser(value_parser!(String)),
        )
        .arg(
            Arg::new("database-url")
                .short('d')
                .long("database-url")
                .help("PostgreSQL database URL")
                .required(false)
                .default_value("postgres://postgres:postgres@localhost:5432/postgres")
                .value_parser(value_parser!(String)),
        )
        .arg(
            Arg::new("bind-host")
                .short('b')
                .long("bind-host")
                .help("Host to bind to")
                .required(false)
                .default_value("127.0.0.1:8080")
                .value_parser(value_parser!(String)),
        )
        .get_matches();

    let config = Configuration {
        layer_path: matches.get_one::<PathBuf>("layer-path").unwrap().into(),
        layer_suffix: matches.get_one::<String>("layer-suffix").unwrap().into(),
    };
    let database_url: String = matches.get_one::<String>("database-url").unwrap().into();
    let bind_host: String = matches.get_one::<String>("bind-host").unwrap().into();

    let pool = PgPoolOptions::new()
        .connect(database_url.as_str())
        .await
        .expect(
            format!("Failed to connect to database, please provide a proper --database-url")
                .as_str(),
        );

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to migrate database");

    println!("Starting server on {} …", bind_host);

    HttpServer::new(move || {
        App::new()
            .wrap(Cors::permissive()) // TODO: Configure CORS
            .wrap(middleware::Compress::default())
            .wrap(Logger::default())
            .wrap(Logger::new("%a %{User-Agent}i"))
            .app_data(web::Data::new(pool.clone())) // Cloning Pool is cheap as it is simply a reference-counted handle to the inner pool state
            .app_data(web::Data::new(config.clone()))
            .service(layer::get_available_layers)
            .service(layer::provide_share_for_layer)
            .service(fs::Files::new("/data", config.layer_path.clone()).show_files_listing())
            .service(fs::Files::new("/", "static/").index_file("index.html"))
    })
    .bind(bind_host)?
    .run()
    .await
}
