use std::sync::Arc;

use crate::config::{ServerContext, Setup};
use crate::common::*;
use crate::io::tls::TlsProvider;
use crate::io::{ConnectionInfo, Handler, HandlerService, Session};
use crate::smtp::Interpretter;
use crate::config::{Component, SingleComponent};

use super::{ExtensionSet, InterptetService, SmtpSession};

mod extensions;
mod starttls;

/// An implementation of ESMTP STARTTLS - RFC 3207 - SMTP Service Extension for Secure SMTP over Transport Layer Security
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct StartTls;

pub type Rfc3207 = StartTls;

impl Setup for StartTls {
    fn setup(&self, ctx: &mut ServerContext) {
        ctx.store.add::<HandlerService>(Arc::new(self.clone()));
    }
}

pub struct TlsService {}
impl Component for TlsService {
    type Target = Arc<dyn TlsProvider + Send + Sync>;
}
impl SingleComponent for TlsService {}

impl Handler for StartTls {
    fn handle<'s, 'a, 'f>(&'s self, session: &'a mut Session) -> S2Fut<'f, Result<()>>
    where
        's: 'f,
        'a: 'f,
    {
        session.store.add::<InterptetService>(Arc::new(
            Interpretter::apply(StartTls).to::<StartTls>().build(),
        ));

        let is_encrypted = session
            .store
            .get_ref::<ConnectionInfo>()
            .map(|c| c.encrypted)
            .unwrap_or_default();

        if !is_encrypted {
            // Add tls if needed and available
            if session.store.get_ref::<TlsService>().is_some() {
                session
                    .store
                    .get_or_compose::<SmtpSession>()
                    .extensions
                    .enable(&ExtensionSet::STARTTLS);
            } else {
                warn!("No TLS provider")
            }
        }
        Box::pin(ready(Ok(())))
    }
}
