use std::marker::PhantomData;
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::Notify;

pub struct ReceiverData<CurrentValue> {
    value: Mutex<Option<CurrentValue>>,
    notify: Notify,
}

pub struct Receiver<CurrentValue>(Arc<ReceiverData<CurrentValue>>);

#[derive(Clone)]
pub struct Sender<NewValue, CurrentValue, UpdateFunc>
where
    UpdateFunc: Fn(&NewValue, &mut Option<CurrentValue>),
{
    channel: Arc<Channel<NewValue, CurrentValue, UpdateFunc>>,
}

struct Channel<NewValue, CurrentValue, UpdateFunc>
where
    UpdateFunc: Fn(&NewValue, &mut Option<CurrentValue>),
{
    receivers: Mutex<Vec<Weak<ReceiverData<CurrentValue>>>>,
    update: UpdateFunc,
    new_value: PhantomData<NewValue>,
}

impl<NewValue, CurrentValue, UpdateFunc> Sender<NewValue, CurrentValue, UpdateFunc>
where
    // Merge new value in to current value (new_value, current_value)
    UpdateFunc: Fn(&NewValue, &mut Option<CurrentValue>),
{
    pub fn send(&self, value: NewValue) {
        let channel = &self.channel;
        let receivers = &mut channel.receivers.lock().unwrap();
        let mut i = 0usize;
        while i < receivers.len() {
            if let Some(recv) = receivers[i].upgrade() {
                if let Ok(mut current) = recv.value.lock() {
                    (channel.update)(&value, &mut current);
                    recv.notify.notify_one();
                }
                i += 1;
            } else {
                receivers.remove(i);
            }
        }
    }

    pub fn subscribe(&self) -> Receiver<CurrentValue> {
        let channel = &self.channel;
        let receivers = &mut channel.receivers.lock().unwrap();
        let new_recv = Arc::new(ReceiverData {
            value: Mutex::new(None),
            notify: Notify::new(),
        });
        receivers.push(Arc::downgrade(&new_recv.clone()));
        return Receiver(new_recv);
    }
}

impl<NewValue, CurrentValue, UpdateFunc> Drop for Sender<NewValue, CurrentValue, UpdateFunc>
where
    UpdateFunc: Fn(&NewValue, &mut Option<CurrentValue>),
{
    fn drop(&mut self) {
        let channel = &self.channel;
        let receivers = &mut channel.receivers.lock().unwrap();
	
        while let Some(recv_weak) = receivers.pop() {
	    if let Some(recv) = recv_weak.upgrade() {
		let notify = &recv.notify;
		drop(recv_weak);
		notify.notify_one();
	    }
        }
    }
}

impl<CurrentValue> Receiver<CurrentValue> {
    async fn recv(&self) -> Option<CurrentValue> {
        loop {
            {
                let data = &self.0;
                let value = &mut data.value.lock().unwrap();
                if let Some(value) = value.take() {
                    return Some(value);
                } else {
                    if Arc::weak_count(&self.0) == 0 {
                        return None; // No senders
                    } else {
                        data.notify.notified() // Get the future before the value is unlocked
                    }
                }
            }
            .await // Wait for notification
        }
    }
}

pub fn channel<UpdateFunc, NewValue, CurrentValue>(
    update_func: UpdateFunc,
) -> (
    Sender<NewValue, CurrentValue, UpdateFunc>,
    Receiver<CurrentValue>,
)
where
    UpdateFunc: Fn(&NewValue, &mut Option<CurrentValue>),
{
    let sender = Sender {
        channel: Arc::new(Channel {
            receivers: Mutex::new(Vec::new()),
            update: update_func,
            new_value: PhantomData,
        }),
    };
    let receiver = sender.subscribe();
    (sender, receiver)
}

#[cfg(test)]
mod test {
    use super::channel;
    use std::collections::BTreeSet;
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn channel_test() {
        let (send, recv1) = channel(|x: &u32, set: &mut Option<BTreeSet<u32>>| {
            if let Some(s) = set {
                s.insert(*x);
            } else {
                let mut s = BTreeSet::new();
                s.insert(*x);
                *set = Some(s);
            }
        });
        let recv2 = send.subscribe();
        send.send(1);
        send.send(2);
        assert_eq!(recv1.recv().await, Some(BTreeSet::from([1, 2])));
        send.send(3);
        assert_eq!(recv1.recv().await, Some(BTreeSet::from([3])));
        assert_eq!(recv2.recv().await, Some(BTreeSet::from([1, 2, 3])));
        let join = tokio::spawn(async move {
            assert_eq!(recv1.recv().await, Some(BTreeSet::from([4])));
        });
        send.send(4);
        join.await;
        assert_eq!(recv2.recv().await, Some(BTreeSet::from([4])));
	drop(send);
	assert_eq!(recv2.recv().await, None);
	assert_eq!(recv2.recv().await, None);
	     
    }
}
