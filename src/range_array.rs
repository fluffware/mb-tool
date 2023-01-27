use std::cmp::Ord;
use std::mem;
use std::ops::Range;

#[derive(Eq, PartialEq, Debug)]
pub struct RangeArray<T>
where
    T: Ord + Clone,
{
    array: Vec<Range<T>>,
}

fn set_max<T>(a: &mut T, b: &T)
where
    T: Ord + Clone,
{
    if *a < *b {
        *a = b.clone()
    }
}

impl<T> RangeArray<T>
where
    T: Ord + Clone,
{
    pub fn new() -> Self {
        Self { array: Vec::new() }
    }

    pub fn union(&mut self, r: &Range<T>) {
        let p = match self.array.binary_search_by(|v| v.start.cmp(&r.start)) {
            Ok(p) => {
                set_max(&mut self.array[p].end, &r.end);
                p
            }
            Err(p) => {
                if p > 0 && self.array[p - 1].end >= r.start {
                    set_max(&mut self.array[p - 1].end, &r.end);
                    p - 1
                } else {
                    if p >= self.array.len() {
                        self.array.push(r.clone());
                        return;
                    } else {
                        self.array.insert(p, r.clone());
                    }
                    p
                }
            }
        };

        // Remove or merge with following ranges that overlap
        let mut i = p + 1;
        let end = self.array[p].end.clone();
        while i < self.array.len() {
            if self.array[i].start > end {
                break;
            }
            if self.array[i].end >= end {
                self.array[p].end = self.array[i].end.clone();
            }
            i += 1;
        }

        if p + 1 < i {
            self.array.drain(p + 1..i);
        }
    }

    pub fn clear_into(&mut self) -> Self {
        let mut clone = Vec::new();
        mem::swap(&mut clone, &mut self.array);
        RangeArray { array: clone }
    }

    pub fn is_empty(&self) -> bool {
        self.array.is_empty()
    }
}

impl<T> Default for RangeArray<T>
where
    T: Ord + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, T> IntoIterator for &'a RangeArray<T>
where
    T: Ord + Clone,
{
    type Item = &'a Range<T>;
    type IntoIter = std::slice::Iter<'a, Range<T>>;
    fn into_iter(self) -> Self::IntoIter {
        self.array.iter()
    }
}

#[test]
fn range_array_test() {
    let mut a = RangeArray::new();
    a.union(&(2..3));
    assert_eq!(a, RangeArray { array: vec! {2..3} });
    a.union(&(0..2));
    assert_eq!(a, RangeArray { array: vec! {0..3} });
    a.union(&(-3..-1));
    assert_eq!(
        a,
        RangeArray {
            array: vec! {-3..-1,0..3}
        }
    );
    a.union(&(-1..-0));
    assert_eq!(
        a,
        RangeArray {
            array: vec! {-3..3}
        }
    );
    a.union(&(5..7));
    assert_eq!(
        a,
        RangeArray {
            array: vec! {-3..3, 5..7}
        }
    );
    a.union(&(4..6));
    assert_eq!(
        a,
        RangeArray {
            array: vec! {-3..3, 4..7}
        }
    );
    a.union(&(2..6));
    assert_eq!(
        a,
        RangeArray {
            array: vec! {-3..7}
        }
    );
    a.union(&(-7..-6));
    a.union(&(9..12));
    a.union(&(-6..10));
    assert_eq!(
        a,
        RangeArray {
            array: vec! {-7..12}
        }
    );
}
