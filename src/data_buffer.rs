use std::collections::{HashMap, VecDeque};
use std::default::Default;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::Duration;

use serial_sensors_proto::types::LinearRangeInfo;
use serial_sensors_proto::versions::Version1DataFrame;
use serial_sensors_proto::{DataFrame, IdentifierCode, SensorData, SensorId};

use crate::fps_counter::FpsCounter;

#[derive(Debug)]
pub struct SensorDataBuffer {
    inner: RwLock<InnerSensorDataBuffer>,
    by_sensor: RwLock<HashMap<SensorId, InnerSensorDataBuffer>>,
}

#[derive(Debug)]
struct InnerSensorDataBuffer {
    sensor_specific: bool,
    capacity: usize,
    data: VecDeque<Version1DataFrame>,
    len: AtomicUsize,
    fps: FpsCounter,
    sequence: AtomicU32,
    num_skipped: AtomicU32,
    calibration: Option<LinearRangeInfo>,
    maker: String,
    product: String,
}

impl Default for SensorDataBuffer {
    fn default() -> Self {
        Self {
            inner: RwLock::new(InnerSensorDataBuffer::new(false)),
            by_sensor: RwLock::new(HashMap::default()),
        }
    }
}

impl InnerSensorDataBuffer {
    fn new(sensor_specific: bool) -> Self {
        Self {
            sensor_specific,
            ..Default::default()
        }
    }
}

impl SensorDataBuffer {
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        let inner = self.inner.read().expect("failed to lock");
        inner.len()
    }

    #[allow(dead_code)]
    pub async fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn capacity(&self) -> usize {
        let inner = self.inner.read().expect("failed to lock");
        inner.capacity
    }

    pub fn num_sensors(&self) -> usize {
        let sensors = self.by_sensor.read().expect("failed to lock");
        sensors.len()
    }

    pub fn enqueue(&self, frame: Version1DataFrame) {
        let mut inner = self.inner.write().expect("failed to lock");
        inner.enqueue(frame.clone());

        // Meta frames need to be rewired. We use a helper function for that.
        let sensor_id = frame.target();
        if sensor_id.tag() == 0 {
            // Skip the board, as it's not a sensor.
            return;
        }

        let mut map = self.by_sensor.write().expect("failed to lock");
        map.entry(sensor_id)
            .and_modify(|entry| entry.enqueue(frame.clone()))
            .or_insert_with(|| {
                let mut buffer = InnerSensorDataBuffer::default();
                buffer.enqueue(frame);
                buffer
            });
    }

    pub fn clone_latest(&self, count: usize, target: &mut Vec<Version1DataFrame>) -> usize {
        let inner = self.inner.read().expect("failed to lock");
        inner.clone_latest(count, target)
    }

    /// Returns the average duration between elements.
    pub fn average_duration(&self) -> Duration {
        let inner = self.inner.read().expect("failed to lock");
        inner.fps.average_duration()
    }

    pub fn get_sensors(&self) -> Vec<SensorId> {
        let map = self.by_sensor.read().expect("failed to lock");
        map.keys().cloned().collect()
    }

    pub fn get_latest_by_sensor(&self, id: &SensorId) -> Option<Version1DataFrame> {
        let map = self.by_sensor.read().expect("failed to lock");
        map.get(id).and_then(|entry| entry.get_latest())
    }

    pub fn get_average_duration_by_sensor(&self, id: &SensorId) -> Option<Duration> {
        let map = self.by_sensor.read().expect("failed to lock");
        map.get(id).map(|entry| entry.average_duration())
    }

    pub fn get_skipped_by_sensor(&self, id: &SensorId) -> u32 {
        let map = self.by_sensor.read().expect("failed to lock");
        map.get(id).map(|entry| entry.skipped()).unwrap_or(0)
    }

    pub fn get_sensor_name(&self, id: &SensorId) -> String {
        let map = self.by_sensor.read().expect("failed to lock");
        map.get(id)
            .map(|entry| entry.product.clone())
            .unwrap_or_default()
    }

    pub fn convert_values(&self, id: &SensorId, values: &mut [f32]) -> bool {
        let map = self.by_sensor.read().expect("failed to lock");
        map.get(id)
            .and_then(|entry| entry.calibration.as_ref())
            .map(|info| {
                for value in values.iter_mut() {
                    *value = info.convert(*value);
                }
                true
            })
            .unwrap_or(false)
    }
}

impl Default for InnerSensorDataBuffer {
    fn default() -> Self {
        let capacity = 100;
        Self {
            sensor_specific: true,
            maker: String::new(),
            product: String::new(),
            capacity,
            data: VecDeque::with_capacity(capacity),
            len: AtomicUsize::new(0),
            fps: FpsCounter::default(),
            sequence: AtomicU32::new(0),
            num_skipped: AtomicU32::new(0),
            calibration: None,
        }
    }
}

impl InnerSensorDataBuffer {
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.len.load(Ordering::SeqCst)
    }

    #[allow(dead_code)]
    pub async fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn enqueue(&mut self, frame: Version1DataFrame) {
        // Sensor-specific buffers do not care about identification frames.
        if self.sensor_specific && frame.is_meta() {
            if let SensorData::LinearRanges(calibration) = frame.value {
                self.calibration = Some(calibration);
            } else if let SensorData::Identification(ident) = frame.value {
                match ident.code {
                    IdentifierCode::Generic => {}
                    IdentifierCode::Maker => {
                        self.maker = String::from(ident.as_str().unwrap_or("").trim())
                    }
                    IdentifierCode::Product => {
                        self.product = String::from(ident.as_str().unwrap_or("").trim())
                    }
                    IdentifierCode::Revision => {}
                }
            }

            return;
        }

        let data = &mut self.data;

        let previous = self.sequence.swap(frame.sensor_sequence, Ordering::SeqCst);
        // If the value didn't increase by one (sensor case) or remain identical (metadata case), count it as a strike.
        if frame.sensor_sequence != previous + 1 && frame.sensor_sequence != previous {
            self.num_skipped.fetch_add(1, Ordering::SeqCst);
        }

        data.push_front(frame);
        let max_len = self.capacity;
        data.truncate(max_len);
        self.len.store(data.len(), Ordering::SeqCst);
        self.fps.mark();
    }

    pub fn clone_latest(&self, count: usize, target: &mut Vec<Version1DataFrame>) -> usize {
        let data = &self.data;
        let length = count.min(data.len());
        target.extend(data.range(..length).cloned());
        length
    }

    pub fn skipped(&self) -> u32 {
        self.num_skipped.load(Ordering::SeqCst)
    }

    /// Returns the average duration between elements.
    #[allow(dead_code)]
    pub fn average_duration(&self) -> Duration {
        self.fps.average_duration()
    }

    /// Gets the latest record.
    pub fn get_latest(&self) -> Option<Version1DataFrame> {
        self.data.front().cloned()
    }
}
