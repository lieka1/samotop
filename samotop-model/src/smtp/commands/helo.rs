use crate::common::*;
use crate::smtp::{SmtpHost, SmtpSessionCommand, SmtpState};

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum SmtpHelo {
    Helo(SmtpHost),
    Ehlo(SmtpHost),
    Lhlo(SmtpHost),
}

impl SmtpSessionCommand for SmtpHelo {
    fn verb(&self) -> &str {
        match self {
            SmtpHelo::Helo(_) => "HELO",
            SmtpHelo::Ehlo(_) => "EHLO",
            SmtpHelo::Lhlo(_) => "LHLO",
        }
    }
    fn apply<'s, 'f, S>(self, mut state: S) -> S2Fut<'f, S>
    where
        S: SmtpState + 's,
        's: 'f,
    {
        let local = state.session().service_name.to_owned();
        let remote = self.host().to_string();
        let is_extended = self.is_extended();
        state.reset_helo(self);
        if is_extended {
            let extensions = state.extensions().iter().map(String::from).collect();
            state.say_ehlo(local, extensions, remote)
        } else {
            state.say_helo(local, remote)
        }

        Box::pin(ready(state))
    }
}

impl SmtpHelo {
    pub fn is_extended(&self) -> bool {
        use self::SmtpHelo::*;
        match self {
            Helo(_) => false,
            Ehlo(_) => true,
            Lhlo(_) => true,
        }
    }
    pub fn host(&self) -> &SmtpHost {
        use self::SmtpHelo::*;
        match self {
            Helo(ref host) => host,
            Ehlo(ref host) => host,
            Lhlo(ref host) => host,
        }
    }
    pub fn name(&self) -> String {
        format!("{}", self.host())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mail::Builder,
        smtp::{SmtpMail, SmtpPath, SmtpStateBase},
    };
    use futures_await_test::async_test;

    #[async_test]
    async fn transaction_gets_reset() {
        let mut set = SmtpStateBase::new(Builder::default());
        set.transaction_mut().id = "someid".to_owned();
        set.transaction_mut().mail = Some(SmtpMail::Mail(SmtpPath::Null, vec![]));
        set.transaction_mut().rcpts.push(SmtpPath::Null);
        set.transaction_mut().extra_headers.insert_str(0, "feeeha");
        let sut = SmtpHelo::Helo(SmtpHost::Domain("wex.xor.ro".to_owned()));
        let res = sut.apply(set).await;
        assert!(res.transaction().is_empty());
    }

    #[async_test]
    async fn helo_is_set() {
        let set = SmtpStateBase::new(Builder::default());
        let sut = SmtpHelo::Helo(SmtpHost::Domain("wex.xor.ro".to_owned()));
        let res = sut.apply(set).await;
        assert_eq!(
            res.session().smtp_helo,
            Some(SmtpHelo::Helo(SmtpHost::Domain("wex.xor.ro".to_owned())))
        );
    }

    #[test]
    fn is_sync_and_send() {
        for i in 0..1 {
            let sut = SmtpHelo::Helo(SmtpHost::Domain("wex.xor.ro".to_owned()));
            let set = SmtpStateBase::new(Builder::default());
            let res = sut.apply(set);
            if i == 0 {
                is_sync(res);
            } else {
                is_send(res);
            }
        }
    }

    fn is_sync<T: Sync>(_subj: T) {}
    fn is_send<T: Send>(_subj: T) {}
    //fn is_static<T: 'static>(_subj: T) {}
}
