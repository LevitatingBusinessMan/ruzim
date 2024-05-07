use std::{cell::OnceCell, io, net::IpAddr, path::PathBuf, str::FromStr, sync::{Arc, OnceLock}};

use tiny_http::{Header, Response, Server, Request};
use clap::Parser;
use zim::{Cluster, Zim};
use std::thread;
use log::{info, debug};
mod logger;

#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// The OpenZim file to serve
    #[arg(short, long, env)]
    zimfile: PathBuf,
    /// The address to bind to
    #[arg(short, long, env, default_value="0.0.0.0")]
    bind: IpAddr,
    /// The port to listen on
    #[arg(short, long, env, default_value="8000")]
    port: u16,
    /// The amount of worker threads
    #[arg(short, long, default_value="4")]
    threads: u16,
}

static ZIMFILE: OnceLock<Zim> = OnceLock::new();

fn main() {
    let args = Args::parse();
    logger::init();
    init_zim(&args.zimfile);

    let server = Server::http((args.bind, args.port)).unwrap();
    let server = Arc::new(server);
    
    let mut handles = Vec::with_capacity(args.threads.into());

    for i in 0..args.threads {
        let server = server.clone();
        let handle = thread::spawn(move || thread_loop(i, &server));
        handles.push(handle);
    }

    // Wait for all threads to die
    for handle in handles {
        handle.join().unwrap();
    }
}

/// The loop of a worker thread
fn thread_loop(id: u16, server: &Server) {
    loop {
        match server.recv() {
            Ok(req) => {
                debug!("t{} {:?}", id, req);
                debug!("{:?}", &req.url()[1..]);
                let result = match req.method() {
                    tiny_http::Method::Get => handle_req(req),
                    //tiny_http::Method::Head => req.respond(R),

                    // Respond with the valid methods
                    tiny_http::Method::Options => req.respond(
                        Response::empty(200).with_header(
                            Header::from_str("Allow: GET, HEAD, OPTIONS").unwrap()
                        )
                    ),
                    _ => req.respond(Response::empty(405)),
                };
                result.unwrap();
            },
            Err(_) => todo!(),
        }

    }
}

fn handle_req(req: Request) -> io::Result<()> {
    let zim = ZIMFILE.get().unwrap();
    let mut res = None;
    for dir in ZIMFILE.get().unwrap().iterate_by_urls() {
        if dir.url == req.url()[1..] {
            match dir.target.unwrap() {
                zim::Target::Redirect(_) => todo!(),
                zim::Target::Cluster(cluster_id, blob_id) => {
                    let cluster = zim.get_cluster(cluster_id).unwrap();
                    let blob = cluster.get_blob(blob_id).unwrap();
                    res = Some(Response::from_data(blob.as_ref()));
                },
            }
        }
    }

    req.respond(res.unwrap()).unwrap();

    Ok(())
}

/// Open the .zim file
fn init_zim(path: &PathBuf) {
    let zim = Zim::new(path).unwrap();
    info!(
        "Serving {:?} with {} articles",
        zim.file_path,
        zim.article_count(),
    );
    ZIMFILE.get_or_init(|| zim);
}
