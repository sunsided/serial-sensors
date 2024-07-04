use std::fmt::Display;
use std::ops::Neg;

use num_traits::ConstZero;
use ratatui::prelude::*;
use serial_sensors_proto::versions::Version1DataFrame;
use serial_sensors_proto::{IdentifierCode, SensorData, SensorId, Vector3Data};

use crate::data_buffer::SensorDataBuffer;

pub fn axis_to_span<'a, V>(value: V, highlight: bool) -> Span<'a>
where
    V: Display + 'a,
{
    let span = Span::styled(format!("{:+4.6}", value), Style::default());
    if highlight {
        span.green()
    } else {
        span.white()
    }
}

pub fn raw_to_span<'a, V>(value: V, highlight: bool) -> Span<'a>
where
    V: Display + 'a,
{
    let span = Span::styled(format!("{:+4.6}", value), Style::default());
    if highlight {
        span.white()
    } else {
        span.gray()
    }
}

pub fn highlight_axis_3<T>(x: T, y: T, z: T) -> (bool, bool, bool)
where
    T: PartialOrd + ConstZero + Neg<Output = T>,
{
    let x = if x > T::ZERO { x } else { -x };
    let y = if y > T::ZERO { y } else { -y };
    let z = if z > T::ZERO { z } else { -z };

    if x > y && x > z {
        (true, false, false)
    } else if y > x && y > z {
        (false, true, false)
    } else if z > x && z > y {
        (false, false, true)
    } else {
        (false, false, false)
    }
}

fn sensor_id<'a>(id: &SensorId) -> Vec<Span<'a>> {
    vec![
        Span::styled(id.tag().to_string(), Style::default().yellow()),
        " ".into(),
        Span::styled(format!("{:02X}", id.id()), Style::default().dim()),
        ":".into(),
        Span::styled(
            format!("{:02X}", id.value_type() as u8),
            Style::default().dim(),
        ),
    ]
}

pub fn frame_data_to_line(
    id: &SensorId,
    receiver: &SensorDataBuffer,
    frame: &Version1DataFrame,
    line: &mut Vec<Span>,
) {
    match frame.value {
        SensorData::AccelerometerI16(vec) => {
            line.extend(format_vec3(id, receiver, vec, "acc"));
        }
        SensorData::MagnetometerI16(vec) => {
            line.extend(format_vec3(id, receiver, vec, "mag"));
        }
        SensorData::TemperatureI16(value) => {
            line.extend(vec![
                Span::styled("temp", Style::default().cyan()),
                " = ".into(),
                axis_to_span(value.value as f32 / 8.0 + 20.0, false), // TODO: Don't assume normalization
                "°C".into(),
            ]);
        }
        _ => {}
    }
}

fn format_vec3<'a, D>(
    id: &SensorId,
    receiver: &SensorDataBuffer,
    vec: D,
    name: &'a str,
) -> Vec<Span<'a>>
where
    D: Into<Vector3Data<i16>>,
{
    let vec = vec.into();
    let (highlight_x, highlight_y, highlight_z) = highlight_axis_3(vec.x, vec.y, vec.z);

    let mut values = [vec.x as f32, vec.y as f32, vec.z as f32];
    let transformed = if receiver.transform_values(id, &mut values) {
        Style::default().green()
    } else {
        Style::default().cyan()
    };

    vec![
        Span::styled(name, transformed),
        " = (".into(),
        axis_to_span(values[0], highlight_x),
        ", ".into(),
        axis_to_span(values[1], highlight_y),
        ", ".into(),
        axis_to_span(values[2], highlight_z),
        ")".into(),
    ]
}

pub fn frame_data_to_line_raw(frame: &Version1DataFrame, line: &mut Vec<Span>) {
    match frame.value {
        SensorData::Identification(ref ident) => {
            line.extend(vec![
                Span::styled("ident:", Style::default().cyan()),
                match ident.code {
                    IdentifierCode::Generic => "generic".into(),
                    IdentifierCode::Maker => "maker".into(),
                    IdentifierCode::Product => "prod".into(),
                    IdentifierCode::Revision => "rev".into(),
                },
                " ".into(),
            ]);

            line.extend(sensor_id(&ident.target));

            line.extend(vec![
                " ".into(),
                Span::styled(
                    String::from(ident.as_str().unwrap_or("(invalid)").trim_end()),
                    Style::default().dim(),
                ),
            ])
        }
        SensorData::AccelerometerI16(vec) => {
            let (highlight_x, highlight_y, highlight_z) = highlight_axis_3(vec.x, vec.y, vec.z);

            line.extend(vec![
                Span::styled("acc", Style::default().cyan()),
                " = (".into(),
                raw_to_span(vec.x, highlight_x),
                ", ".into(),
                raw_to_span(vec.y, highlight_y),
                ", ".into(),
                raw_to_span(vec.z, highlight_z),
                ")".into(),
            ]);
        }
        SensorData::MagnetometerI16(vec) => {
            let (highlight_x, highlight_y, highlight_z) = highlight_axis_3(vec.x, vec.y, vec.z);

            line.extend(vec![
                Span::styled("mag", Style::default().cyan()),
                " = (".into(),
                raw_to_span(vec.x, highlight_x),
                ", ".into(),
                raw_to_span(vec.y, highlight_y),
                ", ".into(),
                raw_to_span(vec.z, highlight_z),
                ")".into(),
            ]);
        }
        SensorData::TemperatureI16(value) => {
            line.extend(vec![
                Span::styled("temp", Style::default().cyan()),
                " = ".into(),
                raw_to_span(value.value, false),
            ]);
        }
        _ => {}
    }
}
