#[macro_use]
pub mod descriptive {
    use std::{
        fmt,
        hash::Hash,
        ops::{Deref, DerefMut},
        sync::Arc,
    };
    #[derive(Clone)]
    pub struct Described<T> {
        inner: T,
        debug: Arc<dyn Fn(&T, &mut fmt::Formatter<'_>) -> fmt::Result>,
        display: Arc<dyn Fn(&T, &mut fmt::Formatter<'_>) -> fmt::Result>,
    }
    impl<T> Described<T> {
        pub fn new(
            inner: T,
            debug: impl 'static + Fn(&T, &mut fmt::Formatter<'_>) -> fmt::Result,
            display: impl 'static + Fn(&T, &mut fmt::Formatter<'_>) -> fmt::Result,
        ) -> Self {
            Self {
                inner,
                debug: Arc::new(move |o, f| {
                    f.write_str("Described(")?;
                    debug(o, f)?;
                    f.write_str(")")
                }),
                display: Arc::new(display),
            }
        }
    }
    impl<D, T> AsRef<T> for Described<D>
    where
        D: AsRef<T>,
    {
        fn as_ref(&self) -> &T {
            self.inner.as_ref()
        }
    }
    impl<T> Deref for Described<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }
    impl<T> DerefMut for Described<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.inner
        }
    }
    impl<T> fmt::Display for Described<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            (self.display)(&self.inner, f)
        }
    }
    impl<T> fmt::Debug for Described<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            (self.debug)(&self.inner, f)
        }
    }
    impl<T> PartialEq for Described<T>
    where
        T: PartialEq,
    {
        fn eq(&self, other: &Self) -> bool {
            self.inner.eq(&other.inner)
        }
    }
    impl<T> Eq for Described<T> where T: Eq {}
    impl<T> Hash for Described<T>
    where
        T: Hash,
    {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.inner.hash(state)
        }
    }
    impl<T> From<T> for Described<T>
    where
        T: fmt::Display + fmt::Debug + 'static,
    {
        fn from(inner: T) -> Self {
            let debug = fmt::Debug::fmt;
            let display = fmt::Display::fmt;
            Self::new(inner, debug, display)
        }
    }
    macro_rules! describe {
        ($expr:expr) => {
            $crate::descriptive::Described::new(
                $expr,
                |_, f| {
                    ::core::fmt::Display::fmt(
                        concat!(
                            stringify!($expr),
                            " @",
                            module_path!(),
                            ":",
                            file!(),
                            ":",
                            line!(),
                            ":",
                            column!()
                        ),
                        f,
                    )
                },
                |_, f| ::core::fmt::Display::fmt(stringify!($expr), f),
            )
        };
    }
}

#[macro_use]
pub mod reafy {
    use core::fmt;
    use std::{marker::PhantomData, ops::Deref};

    use crate::descriptive::Described;

    macro_rules! reaf {
        ($expr:expr) => {
            $crate::reafy::reaf_described(describe!($expr))
        };
    }
    pub fn reaf<'t, T>(what: T) -> Reaf<'t, T>
    where
        T: fmt::Debug + fmt::Display + 'static,
    {
        Reaf(Described::from(what), PhantomData)
    }
    pub fn reaf_described<'t, T>(what: Described<T>) -> Reaf<'t, T> {
        Reaf(what, PhantomData)
    }
    #[derive(Clone, PartialEq, Eq, Hash)]
    pub struct Reaf<'t, T: 't>(Described<T>, PhantomData<&'t ()>);
    impl<T> fmt::Display for Reaf<'_, T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fmt(f)
        }
    }
    impl<T> fmt::Debug for Reaf<'_, T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fmt(f)
        }
    }
    impl<T> Deref for Reaf<'_, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    pub trait Reafy {
        fn reafer<'s>(&'s self) -> Box<dyn Reafer + 's>;
    }
    impl<T> Reafy for Reaf<'_, T>
    where
        T: AsRef<[u8]>,
    {
        fn reafer<'s>(&'s self) -> Box<dyn Reafer + 's> {
            let set: &'s [u8] = self.0.deref().as_ref();
            Box::new(Call(describe!(move |b: &u8| match set.contains(b) {
                true => Ok(ReafSuccess::Match),
                false => Err(ReafError::context("does not contain")),
            })))
        }
    }

    impl<T> Reafer for Call<T>
    where
        T: FnMut(&u8) -> ReafResult,
    {
        fn reaf(&mut self, byte: &u8) -> ReafResult {
            self.0(byte)
        }
    }
    struct Call<T>(Described<T>);
    impl<T> fmt::Display for Call<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fmt(f)
        }
    }
    impl<T> fmt::Debug for Call<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fmt(f)
        }
    }

    pub trait Reafer: fmt::Debug {
        fn reaf(&mut self, byte: &u8) -> ReafResult;
    }
    /// Outcome of one reaf call
    /// Ok(None) => match, incomplete
    /// Ok(Some(true)) => match, done
    /// Ok(Some(false)) => match, open
    /// Err(msg) => mismatch or other error
    pub type ReafResult = Result<ReafSuccess, ReafError>;
    #[derive(Debug, Clone, Eq, PartialEq, Hash)]
    pub enum ReafSuccess {
        Incomplete,
        Match,
        Complete,
    }

    #[derive(Debug)]
    pub struct ReafError {
        context: String,
        cause: Option<Box<dyn std::error::Error + 'static>>,
    }
    impl ReafError {
        pub fn context(context: impl fmt::Display) -> Self {
            Self {
                context: context.to_string(),
                cause: None,
            }
        }
        pub fn new(context: impl fmt::Display, cause: impl std::error::Error + 'static) -> Self {
            Self {
                context: context.to_string(),
                cause: Some(Box::new(cause) as Box<dyn 'static + std::error::Error>),
            }
        }
        pub fn add_context(mut self, context: impl fmt::Display) -> Self {
            if self.context.is_empty() {
                self.context = context.to_string()
            } else {
                self.context = format!("{}, {}", self.context, context)
            }
            self
        }
    }
    impl fmt::Display for ReafError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Reaf error")?;
            if !self.context.is_empty() {
                write!(f, " - ")?;
            }
            write!(f, "{}", self.context)?;
            if let Some(ref cause) = self.cause {
                write!(f, ": {}", cause)?;
            }
            Ok(())
        }
    }
    impl std::error::Error for ReafError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            self.cause.as_deref()
        }
    }
}

use reafy::reaf;
use reafy::Reafy;
struct Any;

fn main() {
    let d = reaf!(Any);

    println!("{}", d);
    println!("{:?}", d);

    let d = reaf!("xxx");

    println!("{:?}", d.reafer().reaf(&b'x'));
    println!("{:?}", d.reafer().reaf(&b'z'));

    println!("{}", d);
    println!("{:?}", d);

    let d = reaf!(|x: &str| println!("{}", x));

    println!("{}", d);
    println!("{:?}", d);
}
