use std::future::Ready;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::stream::FilterMap;
use futures::{Stream, StreamExt};
use pin_project::pin_project;

pub type Dedup<S, T, F> =
    FilterMap<DedupReturnNone<S, T, F>, Ready<Option<T>>, fn(Option<T>) -> Ready<Option<T>>>;

#[pin_project]
pub struct DedupReturnNone<S, T, F> {
    #[pin]
    inner: S,

    prev: Option<T>,
    is_equal: F,
}

impl<S, F> Stream for DedupReturnNone<S, S::Item, F>
where
    S: Stream,
    F: FnMut(&S::Item, &S::Item) -> bool,
{
    type Item = Option<S::Item>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use Poll::{Pending, Ready};

        let this = self.project();
        let current = this.inner.poll_next(cx);
        let Ready(current) = current else {
            return Pending;
        };
        let Some(current) = current else {
            return Ready(this.prev.take().map(Some));
        };
        let prev = take_if(this.prev, |prev| !(this.is_equal)(prev, &current));
        let _ = this.prev.insert(current);
        Ready(Some(prev))
    }
}

fn take_if<T, P>(this: &mut Option<T>, predicate: P) -> Option<T>
where
    P: FnOnce(&mut T) -> bool,
{
    if this.as_mut().map_or(false, predicate) {
        this.take()
    } else {
        None
    }
}

pub(crate) fn dedup_by<S, F>(inner: S, is_equal: F) -> Dedup<S, S::Item, F>
where
    S: Stream,
    F: FnMut(&S::Item, &S::Item) -> bool,
{
    let ready = std::future::ready as fn(Option<S::Item>) -> Ready<Option<S::Item>>;

    StreamExt::filter_map(
        DedupReturnNone {
            inner,
            prev: None,
            is_equal,
        },
        ready,
    )
}

#[cfg(test)]
mod test {
    use futures::executor::block_on;
    use futures::stream;
    use futures::StreamExt;

    use super::dedup_by;

    #[test]
    fn test_dedup_basic() {
        let s = stream::iter([1, 1, 2, 3, 3, 3]);
        let s = dedup_by(s, PartialEq::eq);
        let v: Vec<_> = block_on(s.collect());
        assert_eq!(vec![1, 2, 3], v)
    }

    #[test]
    fn test_dedup_no_dup() {
        let s = stream::iter([1, 2, 3]);
        let s = dedup_by(s, PartialEq::eq);
        let v: Vec<_> = block_on(s.collect());
        assert_eq!(vec![1, 2, 3], v)
    }

    #[test]
    fn test_dedup_singleton() {
        let s = stream::iter([1]);
        let s = dedup_by(s, PartialEq::eq);
        let v: Vec<_> = block_on(s.collect());
        assert_eq!(vec![1], v)
    }

    #[test]
    fn test_dedup_empty() {
        let s = stream::iter(Vec::<i32>::new());
        let s = dedup_by(s, PartialEq::eq);
        let v: Vec<_> = block_on(s.collect());
        assert_eq!(Vec::<i32>::new(), v)
    }
}
