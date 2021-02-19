use std::{fmt, future::Future, time::Duration};
use tokio::time::delay_for;

pub struct CommaSeparatedNoSpace<T>(pub T);
impl<T> fmt::Display for CommaSeparatedNoSpace<T>
where
    T: Clone + IntoIterator,
    T::Item: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut it = self.0.clone().into_iter();
        if let Some(e) = it.next() {
            write!(f, "{}", e)?;
            drop(e);
            for e in it {
                write!(f, ",{}", e)?;
            }
        }
        Ok(())
    }
}

pub struct CommaSeparated<T>(pub T);
impl<T> fmt::Display for CommaSeparated<T>
where
    T: Clone + IntoIterator,
    T::Item: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut it = self.0.clone().into_iter();
        if let Some(e) = it.next() {
            write!(f, "{}", e)?;
            drop(e);
            for e in it {
                write!(f, ", {}", e)?;
            }
        }
        Ok(())
    }
}

pub struct Joined<T>(pub T);
impl<T> fmt::Display for Joined<Option<T>>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(x) = &self.0 {
            x.fmt(f)
        } else {
            Ok(())
        }
    }
}
impl<T1, T2> fmt::Display for Joined<(T1, T2)>
where
    T1: fmt::Display,
    T2: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.0 .0, self.0 .1)
    }
}

pub async fn retry_on_fail<R, T, E: std::fmt::Debug>(mut f: impl FnMut() -> R) -> Result<T, E>
where
    R: Future<Output = Result<T, E>>,
{
    let mut count = 8u32;
    loop {
        match f().await {
            Ok(x) => return Ok(x),
            Err(e) => {
                log::warn!("Attempt failed: {:?}", e);
                count -= 1;
                if count == 0 {
                    log::warn!("Retry limit reached");
                    return Err(e);
                } else {
                    log::warn!("Retrying... (remaining count = {:?})", count);
                }
            }
        }
    }
}

pub async fn retry_on_fail_with_delay<R, T, E: std::fmt::Debug>(
    mut f: impl FnMut() -> R,
) -> Result<T, E>
where
    R: Future<Output = Result<T, E>>,
{
    let mut count = 8u32;
    loop {
        match f().await {
            Ok(x) => return Ok(x),
            Err(e) => {
                log::warn!("Attempt failed: {:?}", e);
                count -= 1;
                if count == 0 {
                    log::warn!("Retry limit reached");
                    return Err(e);
                } else {
                    let delay = (16 >> count).max(1);
                    log::warn!(
                        "Retrying in {} seconds... (remaining count = {:?})",
                        delay,
                        count
                    );

                    delay_for(Duration::from_secs(delay)).await;
                }
            }
        }
    }
}
