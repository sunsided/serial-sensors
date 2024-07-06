use std::fmt::Display;
use std::ops::Neg;

use num_traits::ConstZero;
use ratatui::prelude::*;
use serial_sensors_proto::versions::Version1DataFrame;
use serial_sensors_proto::{IdentifierCode, ScalarData, SensorData, SensorId, Vector3Data};

use crate::data_buffer::SensorDataBuffer;

pub fn axis_to_span<'a, V>(value: V, highlight: Max) -> Span<'a>
where
    V: Display + 'a,
{
    Span::styled(format!("{:+4.6}", value), highlight.to_style())
}

pub fn axis_to_span_int<'a, V>(value: V, highlight: bool) -> Span<'a>
where
    V: Display + 'a,
{
    let span = Span::styled(format!("{:+4}", value), Style::default());
    if highlight {
        span.green()
    } else {
        span.white()
    }
}

pub fn raw_to_span<'a, V>(value: V, highlight: Max) -> Span<'a>
where
    V: Display + 'a,
{
    Span::styled(format!("{:+4.6}", value), highlight.to_style_dim())
}

pub fn highlight_axis_3<T>(x: T, y: T, z: T) -> (Max, Max, Max)
where
    T: PartialOrd + ConstZero + Neg<Output = T>,
{
    // Fake abs.
    let (x, x_pos) = if x > T::ZERO { (x, true) } else { (-x, false) };
    let (y, y_pos) = if y > T::ZERO { (y, true) } else { (-y, false) };
    let (z, z_pos) = if z > T::ZERO { (z, true) } else { (-z, false) };

    if x > y && x > z {
        (Max::Positive.flip_if(!x_pos), Max::None, Max::None)
    } else if y > x && y > z {
        (Max::None, Max::Positive.flip_if(!y_pos), Max::None)
    } else if z > x && z > y {
        (Max::None, Max::None, Max::Positive.flip_if(!z_pos))
    } else {
        (Max::None, Max::None, Max::None)
    }
}

pub enum Max {
    None,
    Negative,
    Positive,
}

impl Max {
    fn to_style(&self) -> Style {
        match self {
            Max::None => Style::default().white(),
            Max::Negative => Style::default().yellow().underlined(),
            Max::Positive => Style::default().green().underlined(),
        }
    }

    fn to_style_dim(&self) -> Style {
        match self {
            Max::None => Style::default().dim(),
            Max::Negative => Style::default().white(),
            Max::Positive => Style::default().white(),
        }
    }

    fn flip_if(self, flip: bool) -> Self {
        match self {
            Max::None => Max::None,
            Max::Negative => {
                if flip {
                    Max::Positive
                } else {
                    Max::Negative
                }
            }
            Max::Positive => {
                if flip {
                    Max::Negative
                } else {
                    Max::Positive
                }
            }
        }
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
    let transformed = if receiver.convert_values(id, &mut values) {
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

fn format_scalar<'a, D>(
    id: &SensorId,
    receiver: &SensorDataBuffer,
    data: D,
    name: &'a str,
) -> Vec<Span<'a>>
where
    D: Into<ScalarData<i16>>,
{
    let scalar = data.into();

    let mut values = [scalar.value as f32];
    let transformed = if receiver.convert_values(id, &mut values) {
        Style::default().green()
    } else {
        Style::default().cyan()
    };

    vec![
        Span::styled(name, transformed),
        " = ".into(),
        axis_to_span(values[0], Max::None),
    ]
}

fn format_scalar_int<'a, D>(
    id: &SensorId,
    receiver: &SensorDataBuffer,
    data: D,
    name: &'a str,
) -> Vec<Span<'a>>
where
    D: Into<ScalarData<i16>>,
{
    let scalar = data.into();

    let mut values = [scalar.value as f32];
    let transformed = if receiver.convert_values(id, &mut values) {
        Style::default().green()
    } else {
        Style::default().cyan()
    };

    vec![
        Span::styled(name, transformed),
        " = ".into(),
        axis_to_span_int(values[0], false),
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
        SensorData::LinearRanges(ref ident) => {
            line.extend(vec![
                Span::styled("transformation data", Style::default().cyan()),
                " ".into(),
            ]);

            line.extend(sensor_id(&ident.target));
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
        SensorData::GyroscopeI16(vec) => {
            let (highlight_x, highlight_y, highlight_z) = highlight_axis_3(vec.x, vec.y, vec.z);

            line.extend(vec![
                Span::styled("gyro", Style::default().cyan()),
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
                raw_to_span(value.value, Max::None),
            ]);
        }
        SensorData::HeadingI16(value) => {
            line.extend(vec![
                Span::styled("heading", Style::default().cyan()),
                " = ".into(),
                raw_to_span(value.value, Max::None),
            ]);
        }
        _ => {}
    }
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
        SensorData::GyroscopeI16(vec) => {
            line.extend(format_vec3(id, receiver, vec, "gyro"));
        }
        SensorData::TemperatureI16(value) => {
            line.extend(format_scalar(id, receiver, value, "temp"));
            line.push("°C".into());
        }
        SensorData::HeadingI16(value) => {
            line.extend(format_scalar_int(id, receiver, value, "heading"));
            line.push("°".into());
        }
        _ => {}
    }
}
