use std::future::Ready;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::stream::{FilterMap, Fuse};
use futures::{Stream, StreamExt};
use pin_project::pin_project;

pub type Dedup<S, T, F> =
    FilterMap<DedupFirstReturnNone<S, T, F>, Ready<Option<T>>, fn(Option<T>) -> Ready<Option<T>>>;

#[pin_project]
pub struct DedupFirstReturnNone<S, T, F> {
    #[pin]
    inner: Fuse<S>,

    prev: Option<T>,
    is_equal: F,
}

impl<S, F> Stream for DedupFirstReturnNone<S, S::Item, F>
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

        if this.prev.is_none() && current.is_none() {
            return Ready(None);
        }

        let equal = matches!(
            (this.prev.as_ref(), current.as_ref()),
            (Some(prev), Some(current)) if (this.is_equal)(prev, current),
        );

        let item = if equal {
            None
        } else {
            mem::replace(this.prev, current)
        };

        Ready(Some(item))
    }
}

pub(crate) fn dedup_first_by<S, F>(inner: S, is_equal: F) -> Dedup<S, S::Item, F>
where
    S: Stream,
    F: FnMut(&S::Item, &S::Item) -> bool,
{
    let ready = std::future::ready as fn(Option<S::Item>) -> Ready<Option<S::Item>>;

    StreamExt::filter_map(
        DedupFirstReturnNone {
            inner: inner.fuse(),
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

    use super::dedup_first_by;

    fn first_equal<A: PartialEq, B>(left: &(A, B), right: &(A, B)) -> bool {
        left.0 == right.0
    }

    #[test]
    fn test_dedup_basic() {
        let s = stream::iter([(1, 1), (1, 2), (2, 1), (3, 1), (3, 2), (3, 3)]);
        let s = dedup_first_by(s, first_equal);
        let v: Vec<_> = block_on(s.collect());
        assert_eq!(vec![(1, 1), (2, 1), (3, 1)], v)
    }

    #[test]
    fn test_dedup_no_dup() {
        let s = stream::iter([1, 2, 3]);
        let s = dedup_first_by(s, PartialEq::eq);
        let v: Vec<_> = block_on(s.collect());
        assert_eq!(vec![1, 2, 3], v)
    }

    #[test]
    fn test_dedup_singleton() {
        let s = stream::iter([1]);
        let s = dedup_first_by(s, PartialEq::eq);
        let v: Vec<_> = block_on(s.collect());
        assert_eq!(vec![1], v)
    }

    #[test]
    fn test_dedup_empty() {
        let s = stream::iter(Vec::<i32>::new());
        let s = dedup_first_by(s, PartialEq::eq);
        let v: Vec<_> = block_on(s.collect());
        assert_eq!(Vec::<i32>::new(), v)
    }
}
