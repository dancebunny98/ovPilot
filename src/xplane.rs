//! Read-only X-Plane 11/12 telemetry using the simulator's UDP RREF protocol.

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use crate::Result;

const DATAREFS: [&str; 4] = [
    "sim/version/xplane_internal_version",
    "sim/flightmodel/position/latitude",
    "sim/flightmodel/position/longitude",
    "sim/flightmodel/position/elevation",
];

/// An X-Plane major release recognized from its internal version dataref.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XPlaneVersion {
    /// X-Plane 11.
    Eleven,
    /// X-Plane 12.
    Twelve,
}

impl XPlaneVersion {
    fn from_internal(value: f32) -> Result<Self> {
        match value as u32 {
            110000..=119999 => Ok(Self::Eleven),
            120000..=129999 => Ok(Self::Twelve),
            _ => Err(format!("unsupported X-Plane internal version: {value}").into()),
        }
    }
}

/// A read-only sample received from a running simulator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XPlaneSnapshot {
    /// Simulator major version.
    pub version: XPlaneVersion,
    /// Own aircraft latitude in degrees.
    pub latitude: f32,
    /// Own aircraft longitude in degrees.
    pub longitude: f32,
    /// Own aircraft altitude above sea level in meters.
    pub altitude_m: f32,
}

/// Queries a running X-Plane instance over its UDP RREF interface.
pub struct XPlaneProbe {
    endpoint: SocketAddr,
    timeout: Duration,
}

impl XPlaneProbe {
    /// Creates a probe for an X-Plane UDP endpoint, normally port 49000.
    pub fn new(endpoint: SocketAddr, timeout: Duration) -> Self {
        Self { endpoint, timeout }
    }

    /// Requests shared X-Plane 11/12 datarefs and waits for one complete sample.
    pub fn sample(&self) -> Result<XPlaneSnapshot> {
        let bind_address = if self.endpoint.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        let socket = UdpSocket::bind(bind_address)?;
        socket.set_read_timeout(Some(Duration::from_millis(250)))?;
        for (index, dataref) in DATAREFS.iter().enumerate() {
            socket.send_to(&request(index as i32, dataref)?, self.endpoint)?;
        }

        let deadline = Instant::now() + self.timeout;
        let mut values = [None; DATAREFS.len()];
        let mut buffer = [0u8; 4096];
        while Instant::now() < deadline {
            match socket.recv_from(&mut buffer) {
                Ok((length, sender)) if sender == self.endpoint => {
                    for (index, value) in parse_response(&buffer[..length])? {
                        if let Some(slot) = values.get_mut(index as usize) {
                            *slot = Some(value);
                        }
                    }
                    if values.iter().all(Option::is_some) {
                        return Ok(XPlaneSnapshot {
                            version: XPlaneVersion::from_internal(values[0].unwrap())?,
                            latitude: values[1].unwrap(),
                            longitude: values[2].unwrap(),
                            altitude_m: values[3].unwrap(),
                        });
                    }
                }
                Ok(_) => continue,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ) =>
                {
                    continue
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(format!(
            "no complete X-Plane 11/12 RREF sample from {}",
            self.endpoint
        )
        .into())
    }
}

fn request(index: i32, dataref: &str) -> Result<[u8; 413]> {
    let bytes = dataref.as_bytes();
    if bytes.len() >= 400 {
        return Err("X-Plane dataref exceeds the 400-byte RREF field".into());
    }
    let mut packet = [0u8; 413];
    packet[..5].copy_from_slice(b"RREF\0");
    packet[5..9].copy_from_slice(&5_i32.to_le_bytes());
    packet[9..13].copy_from_slice(&index.to_le_bytes());
    packet[13..13 + bytes.len()].copy_from_slice(bytes);
    Ok(packet)
}

fn parse_response(packet: &[u8]) -> Result<Vec<(i32, f32)>> {
    if !packet.starts_with(b"RREF,") || !(packet.len() - 5).is_multiple_of(8) {
        return Err("invalid X-Plane RREF response".into());
    }
    Ok(packet[5..]
        .as_chunks::<8>()
        .0
        .iter()
        .map(|entry| {
            let index = i32::from_le_bytes(entry[..4].try_into().unwrap());
            let value = f32::from_le_bytes(entry[4..].try_into().unwrap());
            (index, value)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_has_the_fixed_rref_layout() {
        let packet = request(2, DATAREFS[2]).unwrap();
        assert_eq!(&packet[..5], b"RREF\0");
        assert_eq!(i32::from_le_bytes(packet[5..9].try_into().unwrap()), 5);
        assert_eq!(i32::from_le_bytes(packet[9..13].try_into().unwrap()), 2);
        assert_eq!(&packet[13..13 + DATAREFS[2].len()], DATAREFS[2].as_bytes());
    }

    #[test]
    fn response_preserves_indices_and_values() {
        let mut packet = b"RREF,".to_vec();
        packet.extend(1_i32.to_le_bytes());
        packet.extend(51.5_f32.to_le_bytes());
        assert_eq!(parse_response(&packet).unwrap(), vec![(1, 51.5)]);
        assert!(parse_response(b"RREF,\x01").is_err());
    }

    #[test]
    fn recognizes_both_supported_major_versions() {
        assert_eq!(
            XPlaneVersion::from_internal(115000.0).unwrap(),
            XPlaneVersion::Eleven
        );
        assert_eq!(
            XPlaneVersion::from_internal(120500.0).unwrap(),
            XPlaneVersion::Twelve
        );
        assert!(XPlaneVersion::from_internal(130000.0).is_err());
    }

    #[test]
    fn samples_telemetry_over_udp() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let endpoint = server.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let mut buffer = [0u8; 512];
            let mut peer = None;
            for expected_index in 0..DATAREFS.len() {
                let (size, sender) = server.recv_from(&mut buffer).unwrap();
                assert_eq!(size, 413);
                assert_eq!(
                    i32::from_le_bytes(buffer[9..13].try_into().unwrap()),
                    expected_index as i32
                );
                peer = Some(sender);
            }
            let mut response = b"RREF,".to_vec();
            for (index, value) in [120500.0_f32, 51.5, -0.1, 1234.0].into_iter().enumerate() {
                response.extend((index as i32).to_le_bytes());
                response.extend(value.to_le_bytes());
            }
            server.send_to(&response, peer.unwrap()).unwrap();
        });

        let snapshot = XPlaneProbe::new(endpoint, Duration::from_secs(2))
            .sample()
            .unwrap();
        worker.join().unwrap();
        assert_eq!(snapshot.version, XPlaneVersion::Twelve);
        assert_eq!(snapshot.latitude, 51.5);
        assert_eq!(snapshot.longitude, -0.1);
        assert_eq!(snapshot.altitude_m, 1234.0);
    }
}
