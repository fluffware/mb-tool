use crate::range_array::RangeArray;
#[allow(unused_imports)]
use log::debug;
use std::sync::{Arc, RwLock};
use tokio::sync::Notify;

#[derive(Debug)]
pub struct Base<T> {
    array: Vec<T>,
    observers: Vec<Option<Observer>>,
}

#[derive(Debug)]
struct Observer {
    changed: RangeArray<usize>,
    notify: Arc<Notify>,
}

impl<T> Base<T>
where
    T: Default + Clone,
{
    pub fn new(size: usize) -> Self {
        let mut array = Vec::with_capacity(size);
        array.resize(size, T::default());

        Base {
            array,
            observers: Vec::new(),
        }
    }

    pub fn get_observer(&mut self) -> (usize, Arc<Notify>) {
        let new_observer = Observer {
            changed: RangeArray::new(),
            notify: Arc::new(Notify::new()),
        };
        let notify = new_observer.notify.clone();
        match self.observers.iter().position(|u| u.is_none()) {
            Some(p) => {
                self.observers[p] = Some(new_observer);
                (p, notify)
            }
            None => {
                self.observers.push(Some(new_observer));
                (self.observers.len() - 1, notify)
            }
        }
    }

    pub fn release(&mut self, index: usize) {
        self.observers[index] = None;
    }

    pub fn update(&mut self, start: usize, data: &[T], exclude: usize) {
        self.array[start..start + data.len()].clone_from_slice(data);
        for (index, observer) in self.observers.iter_mut().enumerate() {
            if index != exclude {
                if let Some(observer) = observer {
                    observer.changed.union(&(start..start + data.len()));
                    observer.notify.notify_one();
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct ObservableArray<T>
where
    T: Default + Clone,
{
    base: Arc<RwLock<Base<T>>>,
    index: usize,
    notify: Arc<Notify>,
}

impl<T> Clone for ObservableArray<T>
where
    T: Default + Clone,
{
    fn clone(&self) -> Self {
        let base = self.base.clone();
        let (index, notify) = self.base.write().unwrap().get_observer();
        ObservableArray {
            base,
            index,
            notify,
        }
    }
}

impl<T> Drop for ObservableArray<T>
where
    T: Default + Clone,
{
    fn drop(&mut self) {
        let mut base = self.base.write().unwrap();
        base.release(self.index);
    }
}

impl<T> ObservableArray<T>
where
    T: Default + Clone,
{
    pub fn new(size: usize) -> Self {
        let mut base = Base::<T>::new(size);
        let (index, notify) = base.get_observer();
        debug!("Observer {}", index);
        ObservableArray {
            base: Arc::new(RwLock::new(base)),
            index,
            notify,
        }
    }

    pub async fn updated(&self) -> RangeArray<usize> {
        loop {
            /*  Register for notification before reading updates.
            This way a notification won't be missed if it's triggered after the read but before the await */
            {
                let mut base = self.base.write().unwrap();
                let updates = base.observers[self.index]
                    .as_mut()
                    .unwrap()
                    .changed
                    .clear_into();
                if !updates.is_empty() {
                    return updates;
                }
            }
            self.notify.notified().await;
        }
    }

    pub fn update(&self, start: usize, data: &[T]) {
        let mut base = self.base.write().unwrap();
        base.update(start, data, self.index);
    }

    pub fn get_array<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[T]) -> R,
    {
        let base = self.base.read().unwrap();
        f(&base.array)
    }

    pub fn len(&self) -> usize {
        let base = self.base.read().unwrap();
        base.array.len()
    }

    pub fn is_empty(&self) -> bool {
        let base = self.base.read().unwrap();
        base.array.is_empty()
    }
}
