use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::model::{PerfFileData, PerfHeader, PerfTick};

/// Parses a KovaaK's `.perf` binary file from disk using a zero-dependency Proto3 wire decoder.
pub fn parse_perf_file(path: &Path) -> Result<PerfFileData, String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read perf file {:?}: {e}", path))?;
    parse_perf_bytes(&bytes)
}

/// Parses raw bytes of a KovaaK's `.perf` Proto3 file.
pub fn parse_perf_bytes(bytes: &[u8]) -> Result<PerfFileData, String> {
    let mut reader = ProtoReader::new(bytes);
    let mut header = PerfHeader::default();
    let mut buckets: BTreeMap<i64, PerfTick> = BTreeMap::new();

    while !reader.is_eof() {
        let (field_num, wire_type) = reader.read_tag()?;
        match (field_num, wire_type) {
            (1, 2) => {
                let slice = reader.read_bytes()?;
                header = decode_header(slice)?;
            }
            (2, 2) => {
                let slice = reader.read_bytes()?;
                decode_event_into_buckets(slice, &mut buckets)?;
            }
            _ => {
                reader.skip_field(wire_type)?;
            }
        }
    }

    let mut ticks: Vec<PerfTick> = Vec::with_capacity(buckets.len());
    for (idx, (_, mut tick)) in buckets.into_iter().enumerate() {
        tick.tick_index = idx as u32;
        tick.accuracy = if tick.shots_fired > 0 {
            (tick.shots_hit as f64 / tick.shots_fired as f64) * 100.0
        } else {
            0.0
        };
        ticks.push(tick);
    }

    Ok(PerfFileData { header, ticks })
}

fn decode_header(bytes: &[u8]) -> Result<PerfHeader, String> {
    let mut reader = ProtoReader::new(bytes);
    let mut header = PerfHeader::default();

    while !reader.is_eof() {
        let (field_num, wire_type) = reader.read_tag()?;
        match (field_num, wire_type) {
            (1, 2) => header.scenario_name = reader.read_string()?,
            (2, 2) => header.scenario_hash = reader.read_string()?,
            (3, 0) => header.challenge_start_utc_ms = reader.read_varint()? as i64,
            (4, 0) => header.schema_version = reader.read_varint()? as u32,
            (5, 2) => {
                let cfg_bytes = reader.read_bytes()?;
                decode_scenario_config(cfg_bytes, &mut header)?;
            }
            _ => reader.skip_field(wire_type)?,
        }
    }

    Ok(header)
}

fn decode_scenario_config(bytes: &[u8], header: &mut PerfHeader) -> Result<(), String> {
    let mut reader = ProtoReader::new(bytes);
    while !reader.is_eof() {
        let (field_num, wire_type) = reader.read_tag()?;
        match (field_num, wire_type) {
            (1, 5) => header.duration_s = reader.read_f32()? as f64,
            (1, 1) => header.duration_s = reader.read_f64()?,
            (2, 2) => header.scenario_type = reader.read_string()?,
            (3, 2) => header.bots.push(reader.read_string()?),
            (8, 2) => header.map_name = reader.read_string()?,
            _ => reader.skip_field(wire_type)?,
        }
    }
    Ok(())
}

fn decode_event_into_buckets(
    bytes: &[u8],
    buckets: &mut BTreeMap<i64, PerfTick>,
) -> Result<(), String> {
    let mut reader = ProtoReader::new(bytes);
    let mut time_offset_s: f64 = 0.0;
    let mut payload_fields: Vec<(u32, &[u8])> = Vec::with_capacity(2);

    while !reader.is_eof() {
        let (field_num, wire_type) = reader.read_tag()?;
        match (field_num, wire_type) {
            (1, 5) => time_offset_s = reader.read_f32()? as f64,
            (1, 1) => time_offset_s = reader.read_f64()?,
            (2..=17, 2) => {
                let sub = reader.read_bytes()?;
                payload_fields.push((field_num, sub));
            }
            _ => reader.skip_field(wire_type)?,
        }
    }

    let bucket_key = (time_offset_s * 1000.0).round() as i64;
    let tick = buckets.entry(bucket_key).or_insert_with(|| PerfTick {
        time_offset_s,
        ..PerfTick::default()
    });

    for (field_num, sub) in payload_fields {
        match field_num {
            2 => tick.shots_fired = tick.shots_fired.saturating_add(decode_inner_u32(sub)?),
            3 => tick.shots_hit = tick.shots_hit.saturating_add(decode_inner_u32(sub)?),
            4 => tick.shots_missed = tick.shots_missed.saturating_add(decode_inner_u32(sub)?),
            5 => tick.damage_done += decode_inner_f64(sub)?,
            6 => tick.damage_possible += decode_inner_f64(sub)?,
            7 => tick.score += decode_inner_f64(sub)?,
            8 => tick.kills = tick.kills.saturating_add(decode_inner_u32(sub)?),
            9 => tick.deaths = tick.deaths.saturating_add(decode_inner_u32(sub)?),
            10 => tick.overshots = tick.overshots.saturating_add(decode_inner_u32(sub)?),
            11 => tick.reloads = tick.reloads.saturating_add(decode_inner_u32(sub)?),
            12 => tick.pauses = tick.pauses.saturating_add(decode_inner_u32(sub)?),
            13 => tick.mbs_points += decode_inner_f64(sub)?,
            14 => tick.distance_traveled += decode_inner_f64(sub)?,
            15 => tick.target_size = decode_inner_f64(sub)?,
            16 => tick.target_speed = decode_inner_f64(sub)?,
            17 => tick.random_sens_scale = decode_inner_f64(sub)?,
            _ => {}
        }
    }

    Ok(())
}

/// Reads inner field 1 as `u32` (supports varint, fixed32 float, or fixed64 double).
fn decode_inner_u32(bytes: &[u8]) -> Result<u32, String> {
    let mut reader = ProtoReader::new(bytes);
    let mut val = 0u32;
    while !reader.is_eof() {
        let (field_num, wire_type) = reader.read_tag()?;
        if field_num == 1 {
            match wire_type {
                0 => val = reader.read_varint()? as u32,
                5 => val = reader.read_f32()?.round().max(0.0) as u32,
                1 => val = reader.read_f64()?.round().max(0.0) as u32,
                _ => reader.skip_field(wire_type)?,
            }
        } else {
            reader.skip_field(wire_type)?;
        }
    }
    Ok(val)
}

/// Reads inner field 1 as `f64` (supports fixed32 float, fixed64 double, or varint).
fn decode_inner_f64(bytes: &[u8]) -> Result<f64, String> {
    let mut reader = ProtoReader::new(bytes);
    let mut val = 0.0f64;
    while !reader.is_eof() {
        let (field_num, wire_type) = reader.read_tag()?;
        if field_num == 1 {
            match wire_type {
                5 => val = reader.read_f32()? as f64,
                1 => val = reader.read_f64()?,
                0 => val = reader.read_varint()? as f64,
                _ => reader.skip_field(wire_type)?,
            }
        } else {
            reader.skip_field(wire_type)?;
        }
    }
    Ok(val)
}

/// Minimal, forward-compatible Protobuf (proto3) wire-format reader.
struct ProtoReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ProtoReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn read_varint(&mut self) -> Result<u64, String> {
        let mut result: u64 = 0;
        let mut shift: u32 = 0;
        loop {
            if self.pos >= self.data.len() {
                return Err("Unexpected EOF while reading varint".to_string());
            }
            let byte = self.data[self.pos];
            self.pos += 1;
            result |= ((byte & 0x7F) as u64) << shift;
            if (byte & 0x80) == 0 {
                return Ok(result);
            }
            shift += 7;
            if shift >= 64 {
                return Err("Varint overflow".to_string());
            }
        }
    }

    fn read_tag(&mut self) -> Result<(u32, u8), String> {
        let tag = self.read_varint()?;
        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;
        if field_num == 0 {
            return Err("Invalid protobuf field number 0".to_string());
        }
        Ok((field_num, wire_type))
    }

    fn read_bytes(&mut self) -> Result<&'a [u8], String> {
        let len = self.read_varint()? as usize;
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| "Protobuf length overflow".to_string())?;
        if end > self.data.len() {
            return Err(format!(
                "Protobuf length-delimited field exceeds buffer (need {end}, have {})",
                self.data.len()
            ));
        }
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn read_string(&mut self) -> Result<String, String> {
        let slice = self.read_bytes()?;
        Ok(String::from_utf8_lossy(slice).into_owned())
    }

    fn read_f32(&mut self) -> Result<f32, String> {
        if self.pos + 4 > self.data.len() {
            return Err("Unexpected EOF while reading fixed32".to_string());
        }
        let arr: [u8; 4] = self.data[self.pos..self.pos + 4]
            .try_into()
            .map_err(|_| "fixed32 slice error".to_string())?;
        self.pos += 4;
        Ok(f32::from_le_bytes(arr))
    }

    fn read_f64(&mut self) -> Result<f64, String> {
        if self.pos + 8 > self.data.len() {
            return Err("Unexpected EOF while reading fixed64".to_string());
        }
        let arr: [u8; 8] = self.data[self.pos..self.pos + 8]
            .try_into()
            .map_err(|_| "fixed64 slice error".to_string())?;
        self.pos += 8;
        Ok(f64::from_le_bytes(arr))
    }

    fn skip_field(&mut self, wire_type: u8) -> Result<(), String> {
        match wire_type {
            0 => {
                let _ = self.read_varint()?;
                Ok(())
            }
            1 => {
                if self.pos + 8 > self.data.len() {
                    return Err("Unexpected EOF skipping fixed64".to_string());
                }
                self.pos += 8;
                Ok(())
            }
            2 => {
                let _ = self.read_bytes()?;
                Ok(())
            }
            5 => {
                if self.pos + 4 > self.data.len() {
                    return Err("Unexpected EOF skipping fixed32".to_string());
                }
                self.pos += 4;
                Ok(())
            }
            other => Err(format!("Unsupported protobuf wire type: {other}")),
        }
    }
}
