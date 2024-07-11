use std::fs::File;
use std::io::Read;
use std::path::Path;

#[allow(dead_code)]
pub struct OsuReplay {
    pub(crate) gamemode: Gamemode,
    pub(crate) version: u32,
    pub(crate) beatmap_hash: String,
    pub(crate) player_name: String,
    pub(crate) replay_hash: String,
    pub(crate) count_300: u16,
    pub(crate) count_100: u16,
    pub(crate) count_50: u16,
    pub(crate) count_geki: u16,
    pub(crate) count_katu: u16,
    pub(crate) count_miss: u16,
    pub(crate) score: u32,
    pub(crate) max_combo: u16,
    pub(crate) is_perfect_combo: bool,
    pub(crate) mods: u32,
    pub(crate) life_bar_graph: String,
    pub(crate) timestamp: u64,
    pub(crate) online_score_id: u64,
    pub(crate) additional_mod_info: Option<f64>,
    pub(crate) replay_data: Vec<ReplayData>,
}

pub enum Gamemode {
    Standard = 0,
    Taiko = 1,
    CatchTheBeat = 2,
    Mania = 3,
}

impl OsuReplay {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
        let mut file = File::open(path).unwrap();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();

        let (gamemode, offset) = {
            let (byte, offset) = read_u8(&data, 0);
            (
                match byte {
                    0 => Gamemode::Standard,
                    1 => Gamemode::Taiko,
                    2 => Gamemode::CatchTheBeat,
                    3 => Gamemode::Mania,
                    _ => panic!("Invalid gamemode"),
                },
                offset,
            )
        };

        let (version, offset) = read_u32(&data, offset);
        let (beatmap_hash, offset) = read_string(&data, offset);
        let (player_name, offset) = read_string(&data, offset);
        let (replay_hash, offset) = read_string(&data, offset);
        let (count_300, offset) = read_u16(&data, offset);
        let (count_100, offset) = read_u16(&data, offset);
        let (count_50, offset) = read_u16(&data, offset);
        let (count_geki, offset) = read_u16(&data, offset);
        let (count_katu, offset) = read_u16(&data, offset);
        let (count_miss, offset) = read_u16(&data, offset);
        let (score, offset) = read_u32(&data, offset);
        let (max_combo, offset) = read_u16(&data, offset);
        let (is_perfect_combo, offset) = read_u8(&data, offset);
        let (mods, offset) = read_u32(&data, offset);
        let (life_bar_graph, offset) = read_string(&data, offset);
        let (timestamp, offset) = read_u64(&data, offset);
        let (replay_data_length, mut offset) = read_u32(&data, offset);
        let replay_data_compressed = data[offset..offset + replay_data_length as usize].to_vec();
        offset += replay_data_length as usize;

        let (online_score_id, offset) = read_u64(&data, offset);
        let additional_mod_info = if offset < data.len() {
            let (value, _) = read_u64(&data, offset);
            Some(value as f64)
        } else {
            None
        };

        let replay_data = ReplayData::from_compressed_stream(replay_data_compressed);

        OsuReplay {
            gamemode,
            version,
            beatmap_hash,
            player_name,
            replay_hash,
            count_300,
            count_100,
            count_50,
            count_geki,
            count_katu,
            count_miss,
            score,
            max_combo,
            is_perfect_combo: is_perfect_combo == 1,
            mods,
            life_bar_graph,
            timestamp,
            online_score_id,
            additional_mod_info,
            replay_data,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ReplayData {
    // w 	Long 	Time in milliseconds since the previous action
    pub(crate) time: i64,
    // x 	Float 	x-coordinate of the cursor from 0 - 512
    pub(crate) x: f32,
    // y 	Float 	y-coordinate of the cursor from 0 - 384
    pub(crate) y: f32,
    pub(crate) keys: i32,
    pub(crate) total_time: u64,
}

#[allow(dead_code)]
pub enum Keys {
    M1 = 1,
    M2 = 2,
    K1 = 4,
    K2 = 8,
    SMOKE = 16,
}

impl ReplayData {
    fn from_compressed_stream(replay_data_compressed: Vec<u8>) -> Vec<ReplayData> {
        let mut data = String::new();
        lzma::Reader::from(&replay_data_compressed[..])
            .unwrap()
            .read_to_string(&mut data)
            .unwrap();

        let mut total_time = 0;

        data.split(",")
            .filter(|piece| !piece.is_empty())
            .map(|piece| {
                let mut parts = piece.split("|");
                let data = ReplayData {
                    time: parts.next().unwrap().parse().unwrap(),
                    x: parts.next().unwrap().parse().unwrap(),
                    y: parts.next().unwrap().parse().unwrap(),
                    keys: parts.next().unwrap().parse().unwrap(),
                    total_time,
                };

                if data.time >= 0 {
                    total_time += data.time as u64;
                }

                data
            })
            // Filter out rng seed
            .filter(|data| data.time != -12345)
            .collect()
    }
}

fn read_string(data: &Vec<u8>, offset: usize) -> (String, usize) {
    if data[offset] == 0 {
        return (String::new(), offset + 1);
    }

    let offset_length = offset + 1;
    let (length, offset_str) = read_uleb128(data, offset_length);
    let offset_end = offset_str + length as usize;
    let string = String::from_utf8(data[offset_str..offset_end].to_vec()).unwrap();
    (string, offset_end)
}

fn read_uleb128(data: &Vec<u8>, offset: usize) -> (u64, usize) {
    let mut result = 0;
    let mut shift = 0;
    let mut end = offset;
    loop {
        let byte = data[end];
        end += 1;
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    (result, end)
}

fn read_u8(data: &Vec<u8>, offset: usize) -> (u8, usize) {
    (data[offset], offset + 1)
}

fn read_u16(data: &Vec<u8>, offset: usize) -> (u16, usize) {
    let value = u16::from_le_bytes([data[offset], data[offset + 1]]);
    (value, offset + 2)
}

fn read_u32(data: &Vec<u8>, offset: usize) -> (u32, usize) {
    let value = u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]);
    (value, offset + 4)
}

fn read_u64(data: &Vec<u8>, offset: usize) -> (u64, usize) {
    let value = u64::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
    ]);
    (value, offset + 8)
}
