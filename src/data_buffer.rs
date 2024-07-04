use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};

use serial_sensors_proto::versions::Version1DataFrame;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct SensorDataBuffer {
    capacity: usize,
    data: RwLock<VecDeque<Version1DataFrame>>,
    len: AtomicUsize,
}

impl Default for SensorDataBuffer {
    fn default() -> Self {
        let capacity = 20;
        SensorDataBuffer {
            capacity,
            data: RwLock::new(VecDeque::with_capacity(capacity)),
            len: AtomicUsize::new(0),
        }
    }
}

impl SensorDataBuffer {
    pub fn len(&self) -> usize {
        self.len.load(Ordering::SeqCst)
    }

    pub async fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub async fn enqueue(&self, frame: Version1DataFrame) {
        let mut data = self.data.write().await;
        data.push_front(frame);
        let max_len = data.capacity();
        data.truncate(max_len);
        self.len.store(data.len(), Ordering::SeqCst)
    }

    pub async fn clone_latest(&self, count: usize, target: &mut Vec<Version1DataFrame>) -> usize {
        let data = self.data.read().await;
        let length = count.min(data.len());
        target.extend(data.range(..length).cloned());
        length
    }
}
