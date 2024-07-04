use std::sync::atomic::{AtomicUsize, Ordering};

use serial_sensors_proto::versions::Version1DataFrame;
use tokio::sync::RwLock;

#[derive(Debug, Default)]
pub struct SensorDataBuffer {
    data: RwLock<Vec<Version1DataFrame>>,
    len: AtomicUsize,
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
        data.push(frame);
        self.len.store(data.len(), Ordering::SeqCst)
    }

    pub async fn clone_latest(&self, count: usize, target: &mut Vec<Version1DataFrame>) -> usize {
        let data = self.data.read().await;
        let start = data.len().saturating_sub(count);
        let end = (start + count).min(data.len());
        let length = end - start;
        target.extend_from_slice(&data[start..end]);
        length
    }
}
