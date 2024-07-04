use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::Duration;

use serial_sensors_proto::versions::Version1DataFrame;

use crate::fps_counter::FpsCounter;

#[derive(Debug)]
pub struct SensorDataBuffer {
    capacity: usize,
    data: RwLock<VecDeque<Version1DataFrame>>,
    len: AtomicUsize,
    fps: FpsCounter,
}

impl Default for SensorDataBuffer {
    fn default() -> Self {
        let capacity = 20;
        SensorDataBuffer {
            capacity,
            data: RwLock::new(VecDeque::with_capacity(capacity)),
            len: AtomicUsize::new(0),
            fps: FpsCounter::default(),
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

    pub fn enqueue(&self, frame: Version1DataFrame) {
        let mut data = self.data.write().expect("lock failed");
        data.push_front(frame);
        let max_len = data.capacity();
        data.truncate(max_len);
        self.len.store(data.len(), Ordering::SeqCst);
        self.fps.mark();
    }

    pub fn clone_latest(&self, count: usize, target: &mut Vec<Version1DataFrame>) -> usize {
        let data = self.data.read().expect("lock failed");
        let length = count.min(data.len());
        target.extend(data.range(..length).cloned());
        length
    }

    /// Returns the average duration between elements.
    pub fn average_duration(&self) -> Duration {
        self.fps.average_duration()
    }
}
