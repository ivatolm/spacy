mod server;

use server::Server;

fn main() {
    let server = Server::new();
    let server_handle = server.start();

    match server_handle.join() {
        Ok(_) => {},
        Err(_) => {}
    };
}
