use std::collections::{HashMap, VecDeque};
use std::default::Default;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::Duration;

use serial_sensors_proto::versions::Version1DataFrame;
use serial_sensors_proto::SensorId;

use crate::fps_counter::FpsCounter;

#[derive(Debug)]
pub struct SensorDataBuffer {
    inner: InnerSensorDataBuffer,
    by_sensor: RwLock<HashMap<SensorId, InnerSensorDataBuffer>>,
}

#[derive(Debug)]
struct InnerSensorDataBuffer {
    capacity: usize,
    data: RwLock<VecDeque<Version1DataFrame>>,
    len: AtomicUsize,
    fps: FpsCounter,
}

impl Default for SensorDataBuffer {
    fn default() -> Self {
        Self {
            inner: Default::default(),
            by_sensor: RwLock::new(HashMap::default()),
        }
    }
}

impl Default for InnerSensorDataBuffer {
    fn default() -> Self {
        let capacity = 100;
        Self {
            capacity,
            data: RwLock::new(VecDeque::with_capacity(capacity)),
            len: AtomicUsize::new(0),
            fps: FpsCounter::default(),
        }
    }
}

impl SensorDataBuffer {
    pub fn len(&self) -> usize {
        self.inner.len.load(Ordering::SeqCst)
    }

    pub async fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn capacity(&self) -> usize {
        self.inner.capacity
    }

    pub fn num_sensors(&self) -> usize {
        let sensors = self.by_sensor.read().expect("failed to lock");
        sensors.len()
    }

    pub fn enqueue(&self, frame: Version1DataFrame) {
        let sensor_id = SensorId::from(&frame);
        self.inner.enqueue(frame.clone());

        let mut map = self.by_sensor.write().expect("failed to lock");
        map.entry(sensor_id)
            .and_modify(|entry| entry.enqueue(frame.clone()))
            .or_insert_with(|| {
                let buffer = InnerSensorDataBuffer::default();
                buffer.enqueue(frame);
                buffer
            });
    }

    pub fn clone_latest(&self, count: usize, target: &mut Vec<Version1DataFrame>) -> usize {
        self.inner.clone_latest(count, target)
    }

    /// Returns the average duration between elements.
    pub fn average_duration(&self) -> Duration {
        self.inner.fps.average_duration()
    }
}

impl InnerSensorDataBuffer {
    pub fn len(&self) -> usize {
        self.len.load(Ordering::SeqCst)
    }

    pub async fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn enqueue(&self, frame: Version1DataFrame) {
        let mut data = self.data.write().expect("lock failed");
        data.push_front(frame);
        let max_len = self.capacity;
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
