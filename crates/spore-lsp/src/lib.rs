pub mod server;

pub fn lsp_main() {
    let mut server = server::LspServer::new();
    server.run();
}
