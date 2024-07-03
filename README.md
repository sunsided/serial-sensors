# Serial Sensors

A simple utility program to fetch data off my microcontrollers over a serial connection.
Its primary goal is to provide a host-side implementation of a semi-standardized
protocol for reading sensor data, such as IMU data (accelerometer, magnetometer, gyroscope),
temperature, etc.

I'm currently using it for these projects:

* [`stm32f3disco-rust`](https://github.com/sunsided/stm32f3disco-rust)
  via [`serial-sensors-proto`](https://github.com/sunsided/serial-sensors-proto).

At the moment it doesn't do much, but just dumps out the information as it comes:

```text
In: 31688, 30:25077 43:04 mag = (602, -65, 997)
In: 31689, 30:25077 44:04 temp = 30.25 °C
In: 31690, 25:25078 42:04 acc = (0.01171875, -0.0078125, 1.015625)
In: 31691, 25:25079 42:04 acc = (0.015625, -0.00390625, 1.0234375)
In: 31692, 25:25080 42:04 acc = (0.0078125, -0.0078125, 1.0195313)
In: 31693, 25:25081 42:04 acc = (0.01171875, -0.015625, 1.015625)
In: 31694, 25:25082 42:04 acc = (0.0078125, -0.015625, 1.0039063)
In: 31695, 30:25082 43:04 mag = (600, -62, 995)
In: 31696, 30:25082 44:04 temp = 30.25 °C
In: 31697, 25:25083 42:04 acc = (0.01171875, -0.0078125, 1.0078125)
In: 31698, 25:25084 42:04 acc = (0.01171875, -0.01953125, 1.0117188)
In: 31699, 25:25085 42:04 acc = (0.015625, -0.0234375, 1.0039063)
In: 31700, 25:25086 42:04 acc = (0.015625, -0.015625, 1.0117188)
In: 31701, 25:25087 42:04 acc = (0.02734375, -0.00390625, 1.0078125)
In: 31702, 30:25087 43:04 mag = (604, -62, 1000)
In: 31703, 25:25088 42:04 acc = (0.0078125, -0.0078125, 1.0195313)
In: 31704, 30:25088 44:04 temp = 30.25 °C
In: 31705, 25:25089 42:04 acc = (0.00390625, -0.0078125, 1.0195313)
In: 31706, 25:25090 42:04 acc = (0.0078125, -0.015625, 1.0117188)
In: 31707, 25:25091 42:04 acc = (0.0078125, -0.015625, 1.0195313)
```
