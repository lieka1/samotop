extern crate samotop;

#[test]
fn use_dead_service() {
    let _ = samotop::server::Server::new().serve(samotop::service::tcp::DummyTcpService);
}

#[test]
fn use_samotop_service() {
    let _ = samotop::server::Server::new();
}

#[test]
fn builder_builds_task() {
    let mail = samotop::service::mail::ConsoleMail::new("Aloha");
    let sess = samotop::service::session::StatefulSessionService::new(mail);
    let svc = samotop::service::tcp::SmtpService::new(sess, Default::default());
    let _srv = samotop::server::Server::on("localhost:25").serve(svc);
}
